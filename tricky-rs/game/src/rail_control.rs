//! The rail's control state exactly as the original runs it (board row G1b): `RailSlideControl_Update` 0x1073e0
//! with `RailSlideControl_CommitQuarterTurn` 0x107730 and `RailSlideControl_PlayQueuedClip` 0x107628: the stick's
//! steer and crouch, a quarter turn on the rail (the clip, the board's animation facing, the spin scored), the
//! queued clip played when the turn's clip ends, the rail kind it leaves, and the reset button. Checked bit for bit
//! in the function runner (`tests/rail_control.rs`).

/// The control object (`cRailSlideControl`): +8 the clip queued by a quarter turn (0 none), +0xc the spin turned on
/// this rail (degrees), +0x10 set once the jump button has been seen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RailControl {
    pub queued: u32,
    pub spin: i32,
    pub armed: u32,
}

/// The rider fields it reads and writes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RailRider {
    /// +0x1dc: how the board sits on the rail (1 along, 2 backwards, 3 and 4 across)
    pub kind: u32,
    /// +0x1b4: the stance; +0x46d4 and +0x46e0 the animation's stance and facing
    pub stance: u32,
    pub x46d4: u32,
    pub x46e0: f32,
    /// read: +0x208 the crouch, +0x420 the board class, stats+0x40 the natural stance
    pub crouch: f32,
    pub class: u32,
    pub stats40: u32,
}

/// What the rail control calls, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailCall {
    /// `Boarder_UpdateBoost` 0x11d038(rider, pad bit 4, 0)
    Boost(u32),
    /// `Boarder_SetSteerTargetUnscaled` 0x126ac8(steer bits, rider)
    SteerTarget(u32),
    /// `Boarder_SetCrouchTarget` 0x126c00(crouch bits, rider)
    CrouchTarget(u32),
    /// `Boarder_SetState`(rider, 0xe): the prewind, to jump off
    Prewind,
    /// `AnimCtl_IsClipAtEnd` 0x15ff40(rider+0x46c0, 0), asked
    ClipAtEnd,
    /// `AnimCtl_TurnAllLayers` 0x15eef8(angle bits)
    TurnLayers(u32),
    /// `Score_UpdateSpin` 0x155e00(rider+0x5820, degrees)
    ScoreSpin(i32),
    /// `Boarder_OnClipQueued` 0x106dc0(rider)
    OnClipQueued,
    /// 0x15ef48(rider+0x46c0, 0)
    Anim15ef48,
    /// `AnimCtl_PlayClip` 0x15fcf8(rider+0x46c0, clip, 0, 5)
    PlayClip(u32),
    /// `Boarder_RequestReset`(rider, 101.46)
    Reset,
}

const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);
const PI: f32 = f32::from_bits(0x4049_0fdb);

/// A signed 6-bit stick axis of the pad word at `shift` (as `(int)(pad << shift) >> 26`), / 31.
fn axis(pad: u32, shift: u32) -> f32 {
    ((pad << shift) as i32 >> 26) as f32 / 31.0
}

/// `RailSlideControl_Update` 0x1073e0 on the pad word (`GetControls`' layout); `clip_at_end` answers
/// `AnimCtl_IsClipAtEnd`.
pub fn update(c: &mut RailControl, r: &mut RailRider, pad: u32, clip_at_end: &mut dyn FnMut() -> bool) -> Vec<RailCall> {
    let mut calls = vec![RailCall::Boost((pad as i32 >> 4 & 1) as u32)];
    let mut steer = axis(pad, 0x15);
    if c.armed == 0 {
        if pad & 2 != 0 {
            c.armed = 1;
        }
    } else {
        c.armed = (pad & 6 != 0) as u32;
    }
    if c.queued == 0 {
        let turn = ((pad << 9) as i32 >> 0x1a) as f32 / 31.0;
        if pad & 8 == 0 && turn == 0.0 {
            calls.push(RailCall::SteerTarget(steer.to_bits()));
            calls.push(RailCall::CrouchTarget(axis(pad, 0xf).to_bits()));
            if c.armed != 0 {
                calls.push(RailCall::Prewind);
            }
        } else {
            if turn != 0.0 {
                steer = turn;
            }
            calls.push(RailCall::CrouchTarget(0.0f32.to_bits()));
            if r.crouch == 0.0 && steer != 0.0 {
                commit_quarter_turn(c, r, 0.0 < steer, &mut calls);
            }
        }
    } else {
        calls.push(RailCall::ClipAtEnd);
        if clip_at_end() {
            play_queued_clip(c, r, &mut calls);
        }
    }
    if pad & 1 != 0 {
        calls.push(RailCall::Reset);
    }
    calls
}

/// `RailSlideControl_CommitQuarterTurn` 0x107730: a quarter turn on the rail one way (`right`, mirrored for an
/// alpine board in its natural switch stance): the layers turned ±π/2, ±90° of spin scored, then by the rail kind
/// the facing, stance and clip, and the clip to queue after it.
pub fn commit_quarter_turn(c: &mut RailControl, r: &mut RailRider, right: bool, calls: &mut Vec<RailCall>) {
    let alp = r.class == 2 && r.stats40 != 0;
    let turn = right ^ alp;
    let (layers, step) = if !turn { (HALF_PI, 90) } else { (-HALF_PI, -90) };
    calls.push(RailCall::TurnLayers(layers.to_bits()));
    c.spin += step;
    calls.push(RailCall::ScoreSpin(c.spin));
    calls.push(RailCall::OnClipQueued);
    let (clip, queued) = match (turn, r.kind) {
        (false, 2) | (true, 2) => {
            let face = if alp {
                r.stance = 1;
                if !turn { HALF_PI } else { -HALF_PI }
            } else {
                r.stance = 0;
                r.x46d4 = 0;
                calls.push(RailCall::Anim15ef48);
                if !turn { -HALF_PI } else { HALF_PI }
            };
            r.x46e0 = face;
            if !turn { (0x259, 2) } else { (0x25b, 1) }
        }
        (false, 1) => {
            let face = if alp { -HALF_PI } else { HALF_PI };
            r.x46e0 = face - 0.0;
            (0x25a, 1)
        }
        (false, 3) => {
            r.x46e0 = if alp { -PI } else { 0.0 };
            (0x25c, 3)
        }
        (false, 4) => {
            let (n, a) = if alp { (0.0f32, 0.0f32) } else { (1.0, PI) };
            r.x46e0 = a - n * f32::from_bits(0x40c9_0fdb);
            (0x25f, 4)
        }
        (true, 1) => {
            r.x46e0 = if alp { HALF_PI } else { -HALF_PI };
            (0x258, 2)
        }
        (true, 4) => {
            r.x46e0 = if alp { -PI } else { 0.0 };
            (0x25e, 3)
        }
        (true, 3) => {
            r.x46e0 = if alp { 0.0 } else { -PI };
            (0x25d, 4)
        }
        _ => return,
    };
    calls.push(RailCall::PlayClip(clip));
    c.queued = queued;
}

/// `RailSlideControl_PlayQueuedClip` 0x107628: the clip a quarter turn queued, with the rail kind it leaves (1 →
/// kind 4, 2 → 3, 3 → 1, 4 → 2 with the stance set), then `Boarder_OnClipQueued` and nothing queued.
pub fn play_queued_clip(c: &mut RailControl, r: &mut RailRider, calls: &mut Vec<RailCall>) {
    match c.queued {
        1 => {
            r.kind = 4;
            calls.push(RailCall::PlayClip(0x257));
        }
        2 => {
            r.kind = 3;
            calls.push(RailCall::PlayClip(0x256));
        }
        3 => {
            r.kind = 1;
            calls.push(RailCall::PlayClip(0x254));
        }
        4 => {
            if r.class == 2 {
                r.stance = if r.stats40 == 0 { 1 } else { 0 };
            } else {
                r.x46d4 = 1;
                r.stance = 1;
            }
            r.kind = 2;
            calls.push(RailCall::PlayClip(0x255));
        }
        _ => {}
    }
    calls.push(RailCall::OnClipQueued);
    c.queued = 0;
}

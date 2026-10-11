//! The boarder's motion and control state changes exactly as the original makes them (board row G18):
//! `Boarder_SetMotion` 0x11f610 → `Boarder_ChangeMotionState` 0x11c648 and `Boarder_SetState` 0x11f608 →
//! `Boarder_ChangeControlState` 0x11c7d8, with the switches they run (`Boarder_ExitMotionState` 0x11c6b0,
//! `Boarder_EnterMotionState` 0x11c710, `Boarder_ExitControlState` 0x11c840, `Boarder_EnterControlState` 0x11c8e8).
//! Checked for every old and new value in the function runner (`tests/states.rs`).
//!
//! The state manager (rider+0x5ae0) starts with the rider's address; the motion is rider+0x424, the control state
//! rider+0x428. Each state's object sits in the manager at the offset given here.

/// One state's enter or exit routine: its address, name and the offset of its object in the state manager (the
/// routine's a0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateFn {
    pub addr: u32,
    pub name: &'static str,
    pub obj: u32,
}

const fn f(addr: u32, name: &'static str, obj: u32) -> Option<StateFn> {
    Some(StateFn { addr, name, obj })
}

/// `Boarder_ExitMotionState` 0x11c6b0: the routine leaving motion `m` runs (none for 0, 4 and over 6).
pub fn motion_exit(m: u32) -> Option<StateFn> {
    match m {
        1 => f(0x108368, "AirMotion_Exit", 0x480),
        2 => f(0x10aec8, "GroundMotion_Exit", 0x4b0),
        3 => f(0x10b090, "RailSlideMotion_Exit", 0x4c0),
        5 => f(0x10d680, "WipeoutMotion_Exit", 0x4f0),
        6 => f(0x110290, "WipeOutRecoverMotion_Exit", 0x5f0),
        _ => None,
    }
}

/// `Boarder_EnterMotionState` 0x11c710: the routine entering motion `m` runs (a1 the old motion).
pub fn motion_enter(m: u32) -> Option<StateFn> {
    match m {
        1 => f(0x108310, "AirMotion_Enter", 0x480),
        2 => f(0x109858, "GroundMotion_Enter", 0x4b0),
        3 => f(0x10b068, "RailSlideMotion_Enter", 0x4c0),
        5 => f(0x10c198, "WipeoutMotion_Enter", 0x4f0),
        6 => f(0x110260, "WipeOutRecoverMotion_Enter", 0x5f0),
        _ => None,
    }
}

/// `Boarder_ExitControlState` 0x11c840: the routine leaving control state `s` runs.
pub fn control_exit(s: u32) -> Option<StateFn> {
    match s {
        2 => f(0x104388, "BumpControl_Exit", 0x610),
        3 => f(0x1033c0, "CruiseControl_Exit", 0x630),
        4 => f(0x104178, "ChangeFakieControl_Exit", 0x650),
        5 => f(0x104688, "FakieCruiseControl_Exit", 0x660),
        8 => f(0x106140, "GateAnticipateControl_Exit", 0x6a0),
        0xb => f(0x104790, "LandHardControl_Exit", 0x6e0),
        0xc => f(0x104e38, "LandNormalControl_Exit", 0x6f0),
        0xe => f(0x1055a8, "PrewindControl_Exit", 0x710),
        0xf => f(0x1075f0, "RailSlideControl_Exit", 0x720),
        0x10 => f(0x1029d0, "SpinControl_Exit", 0x740),
        0x14 => f(0x108280, "WipeOutRecoverControl_Exit", 0x7f0),
        0x16 => f(0x106a48, "ResetWaitControl_Exit", 0x810),
        _ => None,
    }
}

/// `Boarder_EnterControlState` 0x11c8e8: the routine entering control state `s` runs (a1 the old state). Entering
/// the reset state (0x16) runs none: it zeroes the reset timer (manager +0x818) inline.
pub fn control_enter(s: u32) -> Option<StateFn> {
    match s {
        2 => f(0x104190, "BumpControl_Enter", 0x610),
        3 => f(0x102fb0, "CruiseControl_Enter", 0x630),
        4 => f(0x103eb8, "ChangeFakieControl_Enter", 0x650),
        6 => f(0x106600, "FinishLineControl_Enter", 0x670),
        7 => f(0x105d80, "Gate2BaseControl_Enter", 0x690),
        8 => f(0x105f48, "GateAnticipateControl_Enter", 0x6a0),
        9 => f(0x106228, "GateLaunchControl_Enter", 0x6c0),
        0xc => f(0x1049f8, "LandNormalControl_Enter", 0x6f0),
        0xd => f(0x1000b8, "NaturalAirControl_Enter", 0x700),
        0xe => f(0x105070, "PrewindControl_Enter", 0x710),
        0xf => f(0x106dc8, "RailSlideControl_Enter", 0x720),
        0x10 => f(0x100598, "SpinControl_Enter", 0x740),
        0x13 => f(0x108110, "WipeOutControl_Enter", 0x7e0),
        0x15 => f(0x106500, "LessonWaitControl_Enter", 0x800),
        _ => None,
    }
}

/// What a change does, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// the old state's exit routine (the field still holds the old value)
    Exit(StateFn),
    /// the new state's enter routine, a1 the old value (the field already holds the new one)
    Enter(StateFn, u32),
    /// entering state 0x16: the reset timer (manager +0x818, `reset::ResetState::timer`) = 0
    ClearResetTimer,
}

/// `Boarder_SetMotion` 0x11f610: nothing if `new` is the motion already; else exit the old, store, enter the new.
pub fn set_motion(motion: &mut u32, new: u32) -> Vec<Change> {
    if new == *motion {
        return Vec::new();
    }
    let old = *motion;
    let mut out: Vec<Change> = motion_exit(old).map(Change::Exit).into_iter().collect();
    *motion = new;
    out.extend(motion_enter(new).map(|e| Change::Enter(e, old)));
    out
}

/// `Boarder_SetState` 0x11f608: as [`set_motion`] for the control state.
pub fn set_state(state: &mut u32, new: u32) -> Vec<Change> {
    if new == *state {
        return Vec::new();
    }
    let old = *state;
    let mut out: Vec<Change> = control_exit(old).map(Change::Exit).into_iter().collect();
    *state = new;
    if new == 0x16 {
        out.push(Change::ClearResetTimer);
    } else {
        out.extend(control_enter(new).map(|e| Change::Enter(e, old)));
    }
    out
}

//! SSX Tricky's riding physics without Bevy (board row F11a): the parts of tricky-rs that tests and the
//! function runner (tools/r5900) need to call directly. tricky-rs re-exports these modules.

pub mod anim;
pub mod collide;
pub mod rails;
pub mod rider;
pub mod trickdata;

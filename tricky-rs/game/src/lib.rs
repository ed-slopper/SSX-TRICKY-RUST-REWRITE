//! SSX Tricky's riding physics without Bevy (board row F11a): the parts of tricky-rs that tests and the
//! function runner (tools/r5900) need to call directly. tricky-rs re-exports these modules.

pub mod air;
pub mod boarder;
pub use tricky_data::anim;
pub mod collide;
pub mod props;
pub mod race;
pub mod rand;
pub mod ground;
pub mod jump;
pub mod landing;
pub mod math;
pub mod rails;
pub mod rivals;
pub mod score;
pub mod rider;
pub mod surface;
pub mod trickdata;

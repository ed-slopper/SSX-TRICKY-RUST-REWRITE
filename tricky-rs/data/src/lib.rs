//! SSX Tricky's file formats and what is read from them, without Bevy (board row F11b): level project
//! folders and their scripts, characters and animation clips, race lines and AI paths, camera scripts,
//! keyframed object curves. `tricky-game` builds on it; the Bevy app `tricky-rs` re-exports it.

pub mod anim;
pub mod character;
pub mod course;
pub mod intro;
pub mod level;
pub mod logic;

//! SSX Tricky's file formats and what is read from them, without Bevy (board row F11b): level project
//! folders and their scripts, characters and animation clips, race lines and AI paths, camera scripts,
//! keyframed object curves. `tricky-game` builds on it; the Bevy app `tricky-rs` re-exports it.

pub mod afl;
pub mod aip;
pub mod anim;
pub mod audiocfg;
pub mod anim_names;
pub mod big;
pub mod character;
pub mod compress;
pub mod course;
pub mod disc;
pub mod ini;
pub mod intro;
pub mod level;
pub mod loc;
pub mod logic;
pub mod mpf;
pub mod music;
pub mod sfn;
pub mod snowsfx;
pub mod ssf;
pub mod ssh;
pub mod trick_names;
pub mod trick_slots;
pub mod trickdef;

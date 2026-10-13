//! The effects' tick (board row G7f): `Boarder_UpdateEffects` 0x1350c0 runs the rider's six effect objects in
//! order, each at its offset in the boarder: the board puff (+0), the carve trail (+0x1c20,
//! [`crate::boardtrail`]), the ground spray (+0x4b60, [`crate::surfacespray`]), the brake spray and fan (+0x99b0,
//! [`crate::brakefx`]), the speed dust (+0xb600, [`crate::puff`]) and, unless the game is split-screen with a
//! window of more than two, the board track (+0x8020, [`crate::boardtrack`]).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Puff,
    Trail,
    Spray,
    Brake,
    Dust,
    Track,
}

/// The effects to run this tick, with their offsets, in order. `players` is the game's player count
/// (`*(*(gApp + 0x730) + 0x298)`), `window` the first screen's field at +0x88 (`*(*(*(gApp + 0x730) + 0x1c) + 0x88)`).
pub fn plan(players: i32, window: i32) -> Vec<(Effect, u32)> {
    let mut v = vec![
        (Effect::Puff, 0),
        (Effect::Trail, 0x1c20),
        (Effect::Spray, 0x4b60),
        (Effect::Brake, 0x99b0),
        (Effect::Dust, 0xb600),
    ];
    if !(players >= 2 && window > 2) {
        v.push((Effect::Track, 0x8020));
    }
    v
}

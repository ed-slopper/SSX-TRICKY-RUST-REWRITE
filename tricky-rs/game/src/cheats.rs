//! The button cheats (board row E10): `cCheatCodes::Update` 0x26cf60 on each front-end player (`cFE3D` vf5
//! 0x265398 calls it every frame), with `FEPlayer_IsPadConnected` 0x26cef8, `FE_RumblePad` 0x26d1c8,
//! `CheatCode_Matches` 0x26cdc8 and `Career_ToggleCharacterCheat` 0x16a998. Hold L1 and R1 and press twelve
//! buttons; let go and the twelve are compared with the codes. Checked in the function runner (`tests/cheats.rs`).

use crate::controls::{CIRCLE, DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT, DPAD_UP, LSHIFT, RSHIFT, SQUARE, TRIANGLE, X};

const R: u32 = DPAD_RIGHT;
const D: u32 = DPAD_DOWN;
const L: u32 = DPAD_LEFT;
const U: u32 = DPAD_UP;
const T: u32 = TRIANGLE;
const C: u32 = CIRCLE;
const S: u32 = SQUARE;

/// The codes (0x347e60, 0x347e90, 0x347ec0: twelve button words each)
pub const UNLOCK_EVERYTHING: [u32; 12] = [X, T, R, C, S, D, T, S, L, C, X, U];
pub const MAX_ATTRIBUTES: [u32; 12] = [T, T, R, T, T, D, X, X, L, X, X, U];
pub const MALLORA: [u32; 12] = [X, X, R, C, C, D, T, T, L, S, S, U];
/// The character codes (0x347ef0, one of them), each toggling its `gCheat_Character` word (0x3352b8)
pub const CHARACTERS: [[u32; 12]; 1] = [[X, X, R, X, X, D, X, X, L, X, X, U]];

/// The chord held while entering a code (L1 + R1)
pub const CHORD: u32 = LSHIFT | RSHIFT;

/// The cheats' switches: `gCheat_UnlockEverything` 0x3352a0, `gCheat_MaxAttrib` 0x3352ac, `gCheat_Mallora`
/// 0x3352b0, `gCheat_Character` 0x3352b8.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cheats {
    pub unlock_everything: u32,
    pub max_attrib: u32,
    pub mallora: u32,
    pub character: [u32; 1],
}

/// A front-end player as the cheats read it: +0 this frame's input word (the device's +0x2c), +4..+0x33 the twelve
/// buttons entered, +0x34 how many (a byte), +0x38 whether codes are taken, +0x40 the device, +0x48 / +0x4c the
/// rumble (on, strength).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FePlayer {
    pub input: u32,
    pub entered: [u32; 12],
    pub count: u8,
    pub takes_codes: u32,
    pub device: u32,
    pub rumble: u32,
    pub rumble_strength: u32,
}

/// The pad device as the cheats read it: +4 connected, +0xc last frame's buttons, +0x10 this frame's, +0x2c the
/// input word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheatPad {
    pub connected: u32,
    pub prev: u32,
    pub buttons: u32,
    pub input: u32,
}

pub trait CheatEnv {
    /// `Input_GetDevice`(gApp+0x24, device)
    fn pad(&self, device: u32) -> CheatPad;
    /// the device's virtuals +0x34(1) and +0x24(1, strength): the rumble
    fn rumble_on(&mut self, device: u32);
    fn rumble(&mut self, device: u32, strength: u32);
    /// the front end's sound object (gApp+0x734 → +0xa4), its virtual +0x54(3): the cheat sound
    fn cheat_sound(&mut self);
}

impl FePlayer {
    /// `FE_RumblePad` 0x26d1c8
    pub fn rumble(&self, env: &mut impl CheatEnv) {
        if self.rumble != 0 {
            env.rumble_on(self.device);
            env.rumble(self.device, self.rumble_strength);
        }
    }

    /// `cCheatCodes::Update` 0x26cf60: with the pad connected, the input word kept (a rumble when it is set). While
    /// codes are taken: holding the chord, each new press (besides the chord) is entered; a thirteenth starts again
    /// from the first; the input word is cleared. Let go after exactly twelve: the first code that matches toggles
    /// its switch (a character code clears the others) and plays the sound; the count starts again either way.
    pub fn update_cheats(&mut self, cheats: &mut Cheats, env: &mut impl CheatEnv) {
        let pad = env.pad(self.device);
        if pad.connected == 0 {
            return;
        }
        self.input = pad.input;
        if self.input != 0 {
            self.rumble(env);
        }
        if self.takes_codes == 0 {
            return;
        }
        if pad.buttons & CHORD == CHORD {
            let new = pad.buttons & !pad.prev & 0xffff_f9ff;
            if new != 0 {
                let mut c = self.count;
                if c == 12 {
                    self.count = 0;
                    c = 0;
                }
                self.count = c.wrapping_add(1);
                self.entered[c as usize] = new;
            }
            self.input = 0;
        } else if self.count == 12 {
            let mut hit = true;
            if self.entered == UNLOCK_EVERYTHING {
                cheats.unlock_everything ^= 1;
            } else if self.entered == MAX_ATTRIBUTES {
                cheats.max_attrib ^= 1;
            } else if self.entered == MALLORA {
                cheats.mallora ^= 1;
            } else if let Some(i) = CHARACTERS.iter().position(|c| *c == self.entered) {
                // `Career_ToggleCharacterCheat` 0x16a998: this one toggled, the others off
                cheats.character[i] ^= 1;
                for (k, c) in cheats.character.iter_mut().enumerate() {
                    if k != i {
                        *c = 0;
                    }
                }
            } else {
                hit = false;
            }
            if hit {
                env.cheat_sound();
            }
            self.count = 0;
        } else {
            self.count = 0;
        }
    }
}

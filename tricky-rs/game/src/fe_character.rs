//! The character select's board and outfit choices (board row G3c): the helpers the 3D character (`cFE3DCharacter`)
//! and the select screens (G9h, G9i) call, on the career's unlock bits and its players' profiles. Checked in the
//! function runner (`tests/fe_character.rs`).
//!
//! A "slot" here is `Career_GetPlayerSlot` 0x164318(k): the career's player slot k (+0x30fc, 0x44 each), whose first
//! byte is the player there. The character's rider (+0xb0) is compared with a player's profile rider
//! (`Career_GetPlayerProfile` 0x164330, the profile's word 0).

use crate::career::Career;
use crate::cheats::Cheats;

/// The character's choice: +0xb0 its rider, +0x158 the board, +0x159 the outfit, +0x15c / +0x160 set when each changes
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Look {
    pub rider: u32,
    pub board: i8,
    pub outfit: i8,
    pub board_changed: u32,
    pub outfit_changed: u32,
}

/// `Career_GetPlayerSlot` 0x164318(k)'s first byte: the player in slot k (any k, as the game reads it)
pub fn slot_player(c: &Career, k: i32) -> i8 {
    c.bytes[(0x30fc + 0x44 * k as isize) as usize] as i8
}

/// Player `p`'s profile word: 0 the rider, 3 the outfit (`Career_GetPlayerOutfit` 0x164dc0), 4 the board (0x164e40)
fn profile(c: &Career, p: i8, w: usize) -> u32 {
    c.profile_word(p, w)
}

/// `FE3DCharSel_IsOutfitBlocked` 0x267858(char, p, q, outfit): player `q` wears the outfit with this character's
/// rider, or the outfit is not unlocked for player `p`
pub fn outfit_blocked(c: &Career, rider: u32, p: i8, q: i8, outfit: i8, cheats: &Cheats) -> bool {
    if profile(c, q, 3) == outfit as i32 as u32 && profile(c, q, 0) == rider {
        return true;
    }
    !c.is_outfit_unlocked(p, outfit as i32 as u32, cheats)
}

/// `FE3DCharSel_IsOutfitTakenBy` 0x267910(char, _, q, outfit): player `q` wears the outfit with this character's rider
pub fn outfit_taken_by(c: &Career, rider: u32, q: i8, outfit: i8) -> bool {
    profile(c, q, 3) == outfit as i32 as u32 && profile(c, q, 0) == rider
}

/// `FE3DCharSel_PickFreeOutfit` 0x2679a0(char, n, outfit, start): with `n` ≤ 0, the outfit if slot n's player has it
/// unlocked, else 0. Otherwise, when none of slots 0..n has a player on this rider, the outfit as given; when one
/// has, (start + 7) mod 7 if any of those slots' players wears outfit 0 on this rider, else 0.
pub fn pick_free_outfit(c: &Career, rider: u32, n: i8, outfit: i8, start: i8, cheats: &Cheats) -> i32 {
    let p = slot_player(c, n as i32);
    if n <= 0 {
        return if c.is_outfit_unlocked(p, outfit as i32 as u32, cheats) { outfit as i32 } else { 0 };
    }
    let mut on_rider = false;
    for k in 0..n as i32 {
        if profile(c, slot_player(c, k), 0) == rider {
            on_rider = true;
        }
    }
    if !on_rider {
        return outfit as i32;
    }
    for k in 0..n as i32 {
        if outfit_taken_by(c, rider, slot_player(c, k), 0) {
            return (start as i32 + 7) % 7;
        }
    }
    0
}

impl Look {
    /// `FE3DCharacter_SetBoard` 0x267c20: a different board noted as changed
    pub fn set_board(&mut self, b: i8) {
        if self.board != b {
            self.board = b;
            self.board_changed = 1;
        }
    }

    /// `FE3DCharacter_SetOutfit` 0x267c48
    pub fn set_outfit(&mut self, o: i8) {
        if self.outfit != o {
            self.outfit = o;
            self.outfit_changed = 1;
        }
    }

    /// `FE3DCharSel_RestoreChoice` 0x267b18(char, n): slot n's player's board and outfit when their profile is on this
    /// rider (else 0 and 0), each kept only if unlocked for that player (else 0); the outfit then through
    /// [`pick_free_outfit`] (start 1); both set.
    pub fn restore_choice(&mut self, c: &Career, n: i8, cheats: &Cheats) {
        let p = slot_player(c, n as i32);
        let (mut board, mut outfit) = (0i8, 0i8);
        if profile(c, p, 0) == self.rider {
            board = profile(c, p, 4) as i8;
            outfit = profile(c, p, 3) as i8;
        }
        if !c.is_board_unlocked(p, board as i32 as u32, cheats) {
            board = 0;
        }
        if !c.is_outfit_unlocked(p, outfit as i32 as u32, cheats) {
            outfit = 0;
        }
        let o = pick_free_outfit(c, self.rider, n, outfit, 1, cheats);
        self.set_board(board);
        self.set_outfit(o as i8);
    }
}

/// `CustomBoard_IsUnlocked` 0x267c70(_, n, board): the board unlocked for slot n's player (`Career_IsUnlockBitSet`)
pub fn board_unlocked(c: &Career, n: i32, board: i8, cheats: &Cheats) -> bool {
    c.is_board_unlocked(slot_player(c, n), board as i32 as u32, cheats)
}

/// `FE3DCharSel_IsOutfitAvailable` 0x267cc0(char, n, outfit): not blocked ([`outfit_blocked`] for slot n's player
/// against each of slots 0..n's) and unlocked for slot n's player
pub fn outfit_available(c: &Career, rider: u32, n: i32, outfit: i8, cheats: &Cheats) -> bool {
    let p = slot_player(c, n);
    for k in 0..n.max(0) {
        if outfit_blocked(c, rider, p, slot_player(c, k), outfit, cheats) {
            return false;
        }
    }
    c.is_outfit_unlocked(p, outfit as i32 as u32, cheats)
}

//! The world sound emitters' small functions (board row G10a2), each the game's and checked in the function runner
//! (`tests/worldemitter.rs`): `WorldEmitter_Clear` 0x22d080, `_FindFree` 0x22d188, `_Find` 0x22d1b8, `_Matches`
//! 0x22bf30, `_Falloff` 0x22bc40, `_Stop` 0x22c5a0, `_StopAll` 0x22d408, `_UpdateAll` 0x22d370.
//!
//! The emitter table is 40 records of 0x50 bytes (20 words) a manager points at with its first word. Word 0 is the
//! state (0 free, 1 playing), +4 / +8 / +0x3c the key `Matches` compares, +0x30 the voice handle (-1 none), +0x34 a
//! "still wanted" flag `UpdateAll` clears every pass, +0x38 which voice engine the handle is in, +0x40 -1 once stopped.

/// Words in an emitter record and the number of records.
pub const WORDS: usize = 20;
pub const COUNT: usize = 40;

/// `WorldEmitter_Clear`: free the record and forget its voice.
pub fn clear(e: &mut [u32]) {
    e[0] = 0;
    e[0x10] = 0xffff_ffff;
    e[0xd] = 0;
    e[0xc] = 0xffff_ffff;
}

/// `WorldEmitter_Matches`: the record's key words (+4, +8, +0x3c) equal `a`, `b`, `c`.
pub fn matches(e: &[u32], a: u32, b: u32, c: u32) -> bool {
    a == e[1] && b == e[2] && c == e[0xf]
}

/// `WorldEmitter_FindFree`: the first record whose state is 0.
pub fn find_free(t: &[u32]) -> Option<usize> {
    (0..COUNT).find(|i| t[i * WORDS] == 0)
}

/// `WorldEmitter_Find`: the first in-use record (state not 0) that matches the key.
pub fn find(t: &[u32], a: u32, b: u32, c: u32) -> Option<usize> {
    (0..COUNT).find(|&i| t[i * WORDS] != 0 && matches(&t[i * WORDS..], a, b, c))
}

/// `WorldEmitter_Falloff`: the volume factor of distance fraction `x` for curve `kind` (0 square, 1 and 3 hyperbolic,
/// 2 linear, 4 squared linear, 5 flat to 0.7 then linear). Another kind returns `f3`, whatever the register held.
pub fn falloff(kind: f32, x: f32, f3: f32) -> f32 {
    let k = if kind.is_nan() { -1 } else { kind as i32 };
    match k {
        0 => 1.0 - x * x,
        1 => 1.0 - x / (1.5 - x * 0.5),
        2 => 1.0 - x,
        3 => {
            let t = 1.0 - x;
            t / (1.5 - t * 0.5)
        }
        4 => {
            let t = 1.0 - x;
            t * t
        }
        5 => {
            if x <= f32::from_bits(0x3f33_3333) {
                1.0
            } else {
                (1.0 - x) / f32::from_bits(0x3e99_999a)
            }
        }
        _ => f3,
    }
}

/// What the emitters call out to: the two voice engines' stop (0x20c2a0, 0x220f98: the global at 0x343050, the
/// handle, the fade 0.25), and the per-record update (0x22bf60).
pub trait Env {
    fn global(&mut self) -> u32;
    fn stop_a(&mut self, global: u32, handle: u32);
    fn stop_b(&mut self, global: u32, handle: u32);
    fn update(&mut self, index: usize);
}

/// `WorldEmitter_Stop`: stop the record's voice in the engine it is in and forget it.
pub fn stop(e: &mut [u32], env: &mut dyn Env) {
    let g = env.global();
    if e[0xe] == 0 {
        env.stop_a(g, e[0xc]);
    } else {
        env.stop_b(g, e[0xc]);
    }
    e[0x10] = 0xffff_ffff;
    e[0xc] = 0xffff_ffff;
}

/// `WorldEmitter_StopAll`: free and stop every playing record.
pub fn stop_all(t: &mut [u32], env: &mut dyn Env) {
    for i in 0..COUNT {
        if t[i * WORDS] == 1 {
            t[i * WORDS] = 0;
            stop(&mut t[i * WORDS..(i + 1) * WORDS], env);
        }
    }
}

/// `WorldEmitter_UpdateAll`: every playing record not marked wanted is freed and stopped, the others updated;
/// the wanted mark is cleared on each.
pub fn update_all(t: &mut [u32], env: &mut dyn Env) {
    for i in 0..COUNT {
        if t[i * WORDS] == 1 {
            if t[i * WORDS + 0xd] == 0 {
                t[i * WORDS] = 0;
                stop(&mut t[i * WORDS..(i + 1) * WORDS], env);
            } else {
                env.update(i);
            }
            t[i * WORDS + 0xd] = 0;
        }
    }
}

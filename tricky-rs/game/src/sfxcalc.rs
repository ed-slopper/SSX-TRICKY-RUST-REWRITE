//! The audio system's small calculations (board row G10a), each the game's and checked in the function runner
//! (`tests/sfxcalc.rs`): `Sfx_SurfaceSoundType` 0x20f908, `Sfx_EvalCurve` 0x20d928, `Sfx_ImpactSpeed` 0x21a6a8,
//! `Audio_AnyRiderNearAhead` 0x20fa80, `Audio_IsNearerPlayer2` 0x20fb68, `Audio_GetRiderListener` 0x20fc60,
//! `Audio_VolumeUp` / `Audio_VolumeDown` 0x211f68 / 0x211f90, `Audio_MusicStateToMode` 0x20d9f0.

use crate::jump::{dot, sub};

type V4 = [f32; 4];

/// `Sfx_SurfaceSoundType`: the sound group of the surface under a rider: `surface` is the audio system's word +0x6e40
/// while the rider's motion (+0x424) is 3 (a rail), else the rider's +0x290. 0 none, 1 snow-like (0, 3, 4), 2 (2),
/// 3 (5, 6), 7 (9, 10, 11), 5 (12), 4 (13), 8 (14), 9 (18, 19); the rest 0.
pub fn surface_sound_type(motion: u32, rail_surface: u32, rider_surface: u32) -> u32 {
    match if motion == 3 { rail_surface } else { rider_surface } {
        0 | 3 | 4 => 1,
        2 => 2,
        5 | 6 => 3,
        9 | 10 | 0xb => 7,
        0xc => 5,
        0xd => 4,
        0xe => 8,
        0x12 | 0x13 => 9,
        _ => 0,
    }
}

/// `Sfx_EvalCurve`: one of the audio system's four-point curves (`xs` and `ys`: four shorts each, the 16-byte row at
/// 0x343098 + 0x10 × slot): the straight line between the two points `x` lies between (integer, truncating); before
/// the first point the first value; past the last point the last value; exactly at the last point 0.
pub fn eval_curve(xs: [i16; 4], ys: [i16; 4], x: i64) -> i64 {
    for i in 0..3 {
        if xs[i] as i64 <= x && x < xs[i + 1] as i64 {
            let run = xs[i + 1] as i32 - xs[i] as i32;
            let y0 = ys[i] as i32;
            return (y0 + ((x as i32 - xs[i] as i32).wrapping_mul(ys[i + 1] as i32 - y0)) / run) as i64;
        }
    }
    if x < xs[0] as i64 {
        ys[0] as i64
    } else if x <= xs[3] as i64 {
        0
    } else {
        ys[3] as i64
    }
}

/// `Sfx_ImpactSpeed`: |(int)(rider+0x320 · rider+0x150)| (the vector unit's dot, x y z), the truncation saturating.
pub fn impact_speed(normal: V4, vel: V4) -> i32 {
    let v = dot(normal, vel) as i32;
    if v < 0 { v.wrapping_neg() } else { v }
}

/// `Audio_AnyRiderNearAhead`: whether any rider (`riders`, their positions in the audio system's list) is within
/// `radius` metres of `at` (cm, ×100) and above it (z).
pub fn any_rider_near_ahead(radius: f32, at: [f32; 3], riders: &[[f32; 3]]) -> bool {
    let k = radius * 100.0;
    let reach = k * k;
    riders.iter().any(|p| {
        let dz = p[2] - at[2];
        ((p[0] - at[0]) * (p[0] - at[0]) + (p[1] - at[1]) * (p[1] - at[1]) + dz * dz) < reach && 0.0 < dz
    })
}

/// `Audio_IsNearerPlayer2`: whether player 2 (the second rider position) is no farther from `at` than player 1.
pub fn is_nearer_player2(p1: V4, p2: V4, at: V4) -> bool {
    let (a, b) = (sub(p1, at), sub(p2, at));
    (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]) <= (a[0] * a[0] + a[1] * a[1] + a[2] * a[2])
}

/// `Audio_GetRiderListener`: which player's listener a rider's sounds go to: 0 with no game; the rider's player word
/// (+0x38) when its +0x41c is 1; 0 with one human; else whether player 2 is the nearer.
pub fn rider_listener(game: bool, flag_41c: u32, player: u32, humans: i64, nearer_player2: impl FnOnce() -> bool) -> u32 {
    if !game {
        return 0;
    }
    if flag_41c == 1 {
        return player;
    }
    if humans != 1 {
        return nearer_player2() as u32;
    }
    0
}

/// `Audio_VolumeUp` / `Audio_VolumeDown`: the volume set is the slot's float (audio system + 0x3848 + 0xc × slot)
/// ± 1/11.
pub fn volume_step(current: f32, up: bool) -> f32 {
    let step = f32::from_bits(0x3dba_2e8c);
    if up { current + step } else { current - step }
}

/// `Audio_MusicStateToMode`: 0 and 1 → 3, 2 → 1, 3 → 4, anything else 5.
pub fn music_state_to_mode(state: i32) -> u32 {
    match state {
        0 | 1 => 3,
        2 => 1,
        3 => 4,
        _ => 5,
    }
}

/// A snow sound program stopped by a zero divisor (the original's `break 7`).
#[derive(Debug, PartialEq, Eq)]
pub struct Trap;

/// The program state the interpreter keeps in its own words: [0] the program, [1..] the stack (the top at
/// `[sp]`, growing up from `[1]`), [0x11] the program counter, [0x12] the stack pointer, [0x13] the accumulator.
pub const PC: usize = 0x11;
pub const SP: usize = 0x12;
pub const ACC: usize = 0x13;

/// The operand of an instruction: four bytes, little-endian.
fn word(prog: &[u8], w: &mut [i32]) -> i32 {
    let mut v = 0u32;
    for k in 0..4 {
        v |= (prog[w[PC] as usize] as u32) << (8 * k);
        w[PC] += 1;
    }
    v as i32
}

/// `FUN_0021ae90` 0x21ae90: the interpreter of the snow sound programs `Sfx_LoadBoardConfig` compiles from SNOW.INF
/// (`tricky_data::snowsfx`). It starts at the first byte with an empty stack (the accumulator keeps what it was) and
/// runs to the 0x7f byte; `input(n)` reads the five inputs (LOAD n, the functions 0x21a6a8, 0x21a6f0, 0x21a710,
/// 0x21a740, 0x21a788 on the rider's sound object); ASSIGNVOL and ASSIGNBEND (0x13, 0x14) write the accumulator to
/// `vol` and `bend`. A byte with no instruction (LOG 5, 0x15.., 0x80..) does nothing. `w` is the state (see above).
pub fn run_snow_program(prog: &[u8], w: &mut [i32], input: &mut dyn FnMut(u32) -> i32, vol: &mut i32, bend: &mut i32) -> Result<(), Trap> {
    w[PC] = 0;
    w[SP] = 0;
    loop {
        let op = prog[w[PC] as usize];
        w[PC] += 1;
        if op >= 0x80 {
            continue;
        }
        match op {
            0 => {
                let n = prog[w[PC] as usize];
                w[PC] += 1;
                if n < 5 {
                    w[ACC] = input(n as u32);
                }
            }
            1 => w[ACC] = word(prog, w),
            2 => {
                let n = word(prog, w);
                if n == 0 {
                    return Err(Trap);
                }
                let q = w[ACC].wrapping_mul(127).wrapping_div(n);
                let x = (q as f32) as i32;
                w[ACC] = x.clamp(0, 0x7f);
            }
            3 => w[ACC] = w[ACC].wrapping_mul(w[ACC]),
            4 => w[ACC] = ((w[ACC] as f32).abs().sqrt()) as i32,
            6 => {
                let n = word(prog, w);
                w[ACC] = w[ACC].wrapping_add(n);
            }
            7 => {
                let n = word(prog, w);
                w[ACC] = w[ACC].wrapping_sub(n);
            }
            8 => {
                let n = word(prog, w);
                w[ACC] = w[ACC].wrapping_mul(n);
            }
            9 => {
                let n = word(prog, w);
                if n == 0 {
                    return Err(Trap);
                }
                w[ACC] = w[ACC].wrapping_div(n);
            }
            10 => w[ACC] = w[ACC].wrapping_neg(),
            11 => {
                if w[ACC] < 0 {
                    w[ACC] = w[ACC].wrapping_neg();
                }
            }
            12 => {
                let lo = word(prog, w);
                let hi = word(prog, w);
                w[ACC] = lo.max(w[ACC].min(hi));
            }
            13 => {
                let sp = w[SP];
                w[(1 + sp) as usize] = w[ACC];
                w[SP] = sp + 1;
            }
            14 => {
                let sp = w[SP];
                w[SP] = sp - 1;
                w[ACC] = w[sp as usize];
            }
            15..=18 => {
                let sp = w[SP];
                let top = w[sp as usize];
                let second = w[(sp - 1) as usize];
                let r = match op {
                    15 => top.wrapping_add(second),
                    16 => top.wrapping_sub(second),
                    17 => top.wrapping_mul(second),
                    _ => {
                        if second == 0 {
                            return Err(Trap);
                        }
                        top.wrapping_div(second)
                    }
                };
                w[(sp - 1) as usize] = r;
                w[SP] = sp - 1;
            }
            0x13 => *vol = w[ACC],
            0x14 => *bend = w[ACC],
            0x7f => return Ok(()),
            _ => {}
        }
    }
}

/// `SndVoice_DistanceVolume` 0x20aab8: the volume (of `volume`) a voice `distance` away has with audible `range`:
/// the distance less half a unit (not below 0), then the squared fraction of the range left; 0 beyond it.
pub fn voice_distance_volume(distance: f32, range: f32, volume: i32) -> i32 {
    let mut d = distance - 0.5;
    if d < 0.0 {
        d = 0.0;
    }
    if d < range && 0.0 < range {
        let t = (range - d) / range;
        return (t * t * volume as f32) as i32;
    }
    0
}

/// `SndVoice_DopplerPitch` 0x20ab48: the pitch factor of a voice closing at `speed` over `unit` (the ratio clamped to
/// +-894.08, the speed of sound scaled by the game's units, then (x + 8000) / 8000).
pub fn voice_doppler_pitch(speed: f32, unit: f32) -> f32 {
    let mut x = speed / unit;
    if x < f32::from_bits(0xc45f_851e) {
        x = f32::from_bits(0xc45f_851e);
    } else if f32::from_bits(0x445f_851e) < x {
        x = f32::from_bits(0x445f_851e);
    }
    (x + 8000.0) / 8000.0
}

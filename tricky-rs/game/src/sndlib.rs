//! Pieces of EA's sound library (board row G10d), each the game's and checked in the function runner
//! (`tests/sndlib.rs`): `SNDVoice_CalcPitchMult` 0x2d5288 and the pitch-ratio function 0x2d51f8 it uses,
//! `SndVoice_SetVolume` 0x20ae28 and `SndVoice_IsDone` 0x20a8e0.

/// The pitch table at 0x351288 read from the executable: byte `i` is about `256 * 2^(i / 256) - 256`. The negative
/// side of the ratio uses the same bytes from the end (the game reads 0x351388 + index).
const RATIO: [u8; 256] = [
    0, 1, 2, 2, 3, 4, 4, 5, 6, 7, 7, 8, 9, 9, 10, 11,
    12, 12, 13, 14, 14, 15, 16, 17, 17, 18, 19, 20, 20, 21, 22, 23,
    23, 24, 25, 26, 26, 27, 28, 29, 30, 30, 31, 32, 33, 33, 34, 35,
    36, 37, 37, 38, 39, 40, 41, 41, 42, 43, 44, 45, 45, 46, 47, 48,
    49, 50, 50, 51, 52, 53, 54, 55, 55, 56, 57, 58, 59, 60, 61, 61,
    62, 63, 64, 65, 66, 67, 67, 68, 69, 70, 71, 72, 73, 74, 75, 75,
    76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 86, 87, 88, 89, 90,
    91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106,
    107, 108, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 119, 120, 121, 122,
    123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138,
    139, 140, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 154, 155, 156,
    157, 158, 159, 160, 161, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174,
    175, 176, 178, 179, 180, 181, 182, 183, 185, 186, 187, 188, 189, 191, 192, 193,
    194, 196, 197, 198, 199, 200, 202, 203, 204, 205, 207, 208, 209, 210, 212, 213,
    214, 216, 217, 218, 219, 221, 222, 223, 225, 226, 227, 229, 230, 231, 232, 234,
    235, 236, 238, 239, 240, 242, 243, 245, 246, 247, 249, 250, 251, 253, 254, 255,
];

/// 0x2d51f8: the playback ratio of a pitch offset in cents (1200 to the octave), 0x1000 being 1.0: whole octaves
/// shift the 0x1000, the remainder is looked up in `RATIO` by `cents * 256 / 1200` (`* 0x369d >> 16`).
pub fn pitch_ratio(cents: i32) -> i32 {
    let mut a = cents;
    let mut r: i32 = 0x1000;
    while a >= 0x4b0 {
        a -= 0x4b0;
        r = r.wrapping_shl(1);
    }
    while a < -0x4af {
        a += 0x4b0;
        r >>= 1;
    }
    let mut i = a.wrapping_mul(0x369d) >> 16;
    if i < -0xff {
        i = -0xff;
    }
    if i < 0 {
        r.wrapping_mul(RATIO[(256 + i) as usize] as i32 + 0x100) >> 9
    } else {
        r.wrapping_mul(RATIO[i as usize] as i32 + 0x100) >> 8
    }
}

/// The words of a voice record `SNDVoice_CalcPitchMult` reads and writes (the record is 0x88 bytes, the pool's
/// pointer at 0x3511f8).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PitchVoice {
    /// +0x2b: the note (signed).
    pub note: i8,
    /// +0x64: the second table's position.
    pub pos: u8,
    /// +0x70: a table the note goes through (0 none).
    pub note_table: u32,
    /// +0x78: a table the position reads (0 none).
    pub mod_table: u32,
    /// +0x7c: how far the position's table bends the pitch.
    pub mod_depth: i16,
    /// +0x7e: how far the note bends the pitch.
    pub note_depth: i16,
    /// +0x80: the base pitch offset in cents.
    pub base: i16,
    /// +0x82: the pitch ratio, kept once worked out (0 means to work it out).
    pub ratio: u16,
    /// +0x84: the volume side's scale.
    pub scale: u16,
    /// +0x86: the result.
    pub mult: i16,
}

/// `SNDVoice_CalcPitchMult`: work the ratio out when it is not kept (the base, bent by the note and by the table at
/// the position, each `(byte - 0x40) * depth >> 6`), then `mult = ratio * scale >> 12`. `byte` reads a signed byte
/// of the game's memory.
pub fn calc_pitch_mult(v: &mut PitchVoice, byte: &dyn Fn(u32) -> i8) {
    if v.ratio == 0 {
        let mut cents = v.base as i32;
        if v.note_depth != 0 {
            let mut n = v.note as i32;
            if v.note_table != 0 {
                n = byte(v.note_table.wrapping_add(n as u32)) as i32;
            }
            cents = cents.wrapping_add((n - 0x40).wrapping_mul(v.note_depth as i32) >> 6);
        }
        if v.mod_table != 0 {
            let n = byte(v.mod_table.wrapping_add(v.pos as u32)) as i32;
            cents = cents.wrapping_add((n - 0x40).wrapping_mul(v.mod_depth as i32) >> 6);
        }
        v.ratio = pitch_ratio(cents) as u16;
    }
    v.mult = ((v.ratio as u32).wrapping_mul(v.scale as u32) as i32 >> 12) as i16;
}

/// The game's `(int)` of a float (`cvt.w.s`: toward zero, out of range or NaN by the sign).
pub fn cvt_w(f: f32) -> i32 {
    let bits = f.to_bits();
    if (bits >> 23) & 0xff >= 158 {
        if bits >> 31 != 0 {
            i32::MIN
        } else {
            i32::MAX
        }
    } else {
        f as i32
    }
}

/// The three engines a `SndVoice` is in: 1 and 2 are EA's two channel libraries (0x2d3xxx and 0x2d4xxx), 3 a
/// stream (0x229xxx). Their calls, with the lock the first two are made under (0x2d0908 / 0x2d0938).
pub trait VoiceEnv {
    fn lock(&mut self);
    fn unlock(&mut self);
    /// 0x2d3260 (the handle).
    fn channel1_reset(&mut self, handle: u32);
    /// 0x2d3108 (the handle, the volume).
    fn channel1_volume(&mut self, handle: u32, volume: i32);
    /// 0x2d4368 (the handle).
    fn channel2_reset(&mut self, handle: u32);
    /// 0x2d4888 (the handle, the volume).
    fn channel2_volume(&mut self, handle: u32, volume: i32);
    /// `Stream_Stop` 0x229868 (the stream).
    fn stream_stop(&mut self, stream: u32);
    /// `Stream_SetVolume` 0x229398 (the stream, the volume).
    fn stream_volume(&mut self, stream: u32, volume: i32);
    /// 0x2d3338 (the handle): non-zero while it plays.
    fn channel1_playing(&mut self, handle: u32) -> i64;
    /// 0x2d4a08 (the handle): the state, 2 done, negative gone.
    fn channel2_state(&mut self, handle: u32) -> i64;
    /// 0x2d2c10 (the handle): the state (3 done) or a negative result.
    fn stream_state(&mut self, handle: u32) -> (i64, i32);
}

/// The words of a `SndVoice` the volume setter reads and writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SndVoice {
    /// +0: which engine (1, 2, 3).
    pub kind: i32,
    /// +4: the channel handle.
    pub handle: u32,
    /// +0xc: the stream.
    pub stream: u32,
    /// +0x90: the volume scale (negative: none).
    pub scale: f32,
    /// +0xa8: the volume last sent (negative: none yet).
    pub last: i32,
}

/// `SndVoice_SetVolume` 0x20ae28: the volume (-1 asks to stop) scaled by the voice's own scale when it has one,
/// clamped to 0..127, and sent to the engine it is in unless it is within four of what was last sent.
pub fn set_volume(v: &mut SndVoice, volume: i32, env: &mut dyn VoiceEnv) {
    if !(1..=3).contains(&v.kind) {
        return;
    }
    let mut s = volume;
    // (the hardware reads a denormal as zero)
    let scale = if v.scale.to_bits() & 0x7f80_0000 == 0 { 0.0 } else { v.scale };
    if 0.0 <= scale {
        s = cvt_w(scale * volume as f32);
    }
    if s >= 0x80 {
        s = 0x7f;
    }
    if s < 0 {
        s = 0;
    }
    if v.last >= 0 && (s.wrapping_sub(v.last).wrapping_add(4) as u32) < 9 {
        return;
    }
    v.last = s;
    match v.kind {
        1 => {
            env.lock();
            if volume == -1 {
                env.channel1_reset(v.handle);
            } else {
                env.channel1_volume(v.handle, s);
            }
            env.unlock();
        }
        2 => {
            env.lock();
            if volume == -1 {
                env.channel2_reset(v.handle);
            } else {
                env.channel2_volume(v.handle, s);
            }
            env.unlock();
        }
        _ => {
            if volume == -1 {
                env.stream_stop(v.stream);
            } else {
                env.stream_volume(v.stream, s);
            }
        }
    }
}

/// `SndVoice_IsDone` 0x20a8e0: the voice has finished (or is gone, or is of no known engine).
pub fn is_done(kind: i32, handle: u32, env: &mut dyn VoiceEnv) -> bool {
    match kind {
        2 => {
            env.lock();
            let r = env.channel2_state(handle);
            env.unlock();
            r == 2 || r < 0
        }
        1 => {
            env.lock();
            let r = env.channel1_playing(handle);
            env.unlock();
            r != 0
        }
        3 => {
            env.lock();
            let (r, state) = env.stream_state(handle);
            env.unlock();
            r < 0 || state == 3
        }
        _ => true,
    }
}

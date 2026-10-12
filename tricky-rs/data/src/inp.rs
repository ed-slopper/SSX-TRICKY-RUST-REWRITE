//! The Trick Tutorial's recorded pad input: `data\tutorial\<board><NN>.inp` (board row C11c).
//!
//! Proven from `cLessonPreviewHandler::Enter` 0x14e138 (reads the file into the lesson manager's 0x4d598-byte
//! buffer, zeroes word 0), the lesson hooks in `cPlayer::GetControls` (the code around 0x152694..0x1526d4) (`FUN_0014b7c8` 0x14b7c8 before
//! the controls are read, `FUN_0014b8a8` 0x14b8a8 after, `FUN_0014b958` 0x14b958) and the preview's end test
//! (`cLessonPreviewHandler::Update` 0x14e270: `buf[0] < buf[1]`):
//!
//! - Header, six words: `[0]` the playback position (a frame index; the game zeroes it after loading), `[1]` the
//!   number of frames. Words 2..5 are not read by any lesson code (kept as `rest`).
//! - Then one 44-byte record per frame, up to 7200 (120 s at 60 Hz: 24 + 7200 x 44 = 0x4d598): a 12-byte control
//!   record (the three words `GetControls` fills for a recording: control state, the word before it, the
//!   controls) and the 32-byte state of the pad device (previous buttons +0xc, buttons +0x10, six stick floats
//!   +0x14..+0x28; `Pad_ReadAxis` 0x179940 reads them).
//!
//! The preview (state 5) plays the recording by swapping the pad's state for the frame's before the controls
//! are read, and putting the live state back after; Perform (state 7) only counts the position up.
//! Not checked against a real file (the disc's `data/tutorial` is not in the repository's inputs): the sizes
//! add up to the buffer size, the 7200 frames, and the field order within a record is the one the code reads
//! (the pad state at record offset 12).

/// The state of the pad device that a frame carries.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PadState {
    /// Buttons of the previous tick (+0xc) and of this one (+0x10).
    pub prev: u32,
    pub now: u32,
    /// The six stick values (+0x14..+0x28).
    pub axes: [f32; 6],
}

/// One recorded frame.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Frame {
    /// The control record: control state, the word before it, the controls.
    pub ctl: [u32; 3],
    pub pad: PadState,
}

pub const HEADER_BYTES: usize = 24;
pub const FRAME_BYTES: usize = 44;
pub const MAX_FRAMES: usize = 7200;
/// The buffer the game loads a file into.
pub const BUFFER_BYTES: usize = HEADER_BYTES + MAX_FRAMES * FRAME_BYTES;

#[derive(Clone, Debug, PartialEq)]
pub struct Inp {
    /// Words 2..5 of the header.
    pub rest: [u32; 4],
    pub frames: Vec<Frame>,
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

impl Inp {
    /// Parse a file; `None` if it is shorter than its frame count says or the count is over 7200.
    pub fn parse(b: &[u8]) -> Option<Inp> {
        if b.len() < HEADER_BYTES {
            return None;
        }
        let n = u32_at(b, 4) as usize;
        if n > MAX_FRAMES || b.len() < HEADER_BYTES + n * FRAME_BYTES {
            return None;
        }
        let mut frames = Vec::with_capacity(n);
        for i in 0..n {
            let o = HEADER_BYTES + i * FRAME_BYTES;
            let w = |k: usize| u32_at(b, o + 4 * k);
            let f = |k: usize| f32::from_bits(w(k));
            frames.push(Frame {
                ctl: [w(0), w(1), w(2)],
                pad: PadState { prev: w(3), now: w(4), axes: [f(5), f(6), f(7), f(8), f(9), f(10)] },
            });
        }
        Some(Inp { rest: [u32_at(b, 8), u32_at(b, 12), u32_at(b, 16), u32_at(b, 20)], frames })
    }

    pub fn len(&self) -> u32 {
        self.frames.len() as u32
    }
}

/// The playback cursor: the lesson buffer's word 0 and the hooks of `GetControls`.
#[derive(Clone, Debug, Default)]
pub struct Playback {
    pub pos: u32,
}

impl Playback {
    /// `FUN_0014b7c8` (preview only): the pad state to put in before the controls are read, the frame at the
    /// position. `None` once the position is at the end (the original then reads past the last frame; the
    /// preview ends on that tick, so the value is not used).
    pub fn pad_for(&self, inp: &Inp) -> Option<PadState> {
        inp.frames.get(self.pos as usize).map(|f| f.pad)
    }

    /// `FUN_0014b8a8`, after the controls (preview and perform): the position advances while below the count.
    pub fn after_controls(&mut self, inp: &Inp) {
        if self.pos < inp.len() {
            self.pos += 1;
        }
    }

    /// The preview is over when the position reaches the count (`cLessonPreviewHandler::Update`).
    pub fn finished(&self, inp: &Inp) -> bool {
        self.pos >= inp.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(frames: &[Frame]) -> Vec<u8> {
        let mut b = vec![0u8; HEADER_BYTES];
        b[0..4].copy_from_slice(&7u32.to_le_bytes()); // the position a recorder left: ignored
        b[4..8].copy_from_slice(&(frames.len() as u32).to_le_bytes());
        for f in frames {
            for w in f.ctl {
                b.extend(w.to_le_bytes());
            }
            b.extend(f.pad.prev.to_le_bytes());
            b.extend(f.pad.now.to_le_bytes());
            for a in f.pad.axes {
                b.extend(a.to_le_bytes());
            }
        }
        b
    }

    #[test]
    fn the_sizes_add_up_to_the_buffer() {
        assert_eq!(BUFFER_BYTES, 0x4d598);
        assert_eq!(FRAME_BYTES, 12 + 32);
    }

    #[test]
    fn parses_and_plays() {
        let fr = [
            Frame { ctl: [1, 2, 3], pad: PadState { prev: 0, now: 0x800, axes: [0.5, -1.0, 0.0, 0.0, 1.0, 0.25] } },
            Frame { ctl: [4, 5, 6], pad: PadState { prev: 0x800, now: 0, axes: [0.0; 6] } },
        ];
        let inp = Inp::parse(&file(&fr)).unwrap();
        assert_eq!(inp.frames, fr);
        let mut p = Playback::default();
        assert_eq!(p.pad_for(&inp), Some(fr[0].pad));
        assert!(!p.finished(&inp));
        p.after_controls(&inp);
        p.after_controls(&inp);
        p.after_controls(&inp); // stops at the count
        assert_eq!(p.pos, 2);
        assert!(p.finished(&inp));
        assert_eq!(p.pad_for(&inp), None);
    }

    #[test]
    fn rejects_short_and_oversized() {
        assert!(Inp::parse(&[0; 10]).is_none());
        let mut b = vec![0u8; HEADER_BYTES];
        b[4..8].copy_from_slice(&3u32.to_le_bytes());
        assert!(Inp::parse(&b).is_none());
        b[4..8].copy_from_slice(&7201u32.to_le_bytes());
        assert!(Inp::parse(&b).is_none());
    }
}

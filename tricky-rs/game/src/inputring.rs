//! The input device list and its ring buffer (row E10c): `Input_Init` 0x17a420, `Input_ReadDevices` 0x17a668,
//! `InputRecorder_Playback` 0x17a740, `Input_FindPressedDevice` 0x17a840, `Input_GetDevicePressed` 0x17a898,
//! `Input_ResetDevices` 0x17a8b0, `Input_GetAnyNewButtons` 0x1834a8, `Pad_LatchButtons` 0x17a658, and the
//! `cPS2Device` pieces around them (vf1, vf2, vf8, vf9's record and motor tail, vf10 over [`PadDevice::read`]).
//!
//! The list (`gApp+0x24`): +0 the ring's buffer, +4 the size of one frame (the devices' record sizes added, 0x58
//! for the two pads), +8 the frames held (8), +0xc the read index, +0x10 the write index, +0x14 the device count,
//! +0x18.. the device pointers. Reading the devices writes one frame at the write index and steps it; playing
//! back gives one frame at the read index to the devices and steps that; the ring is empty when the two are equal
//! (there is no full check: a write that catches the read index makes it look empty).

use crate::controls::PadDevice;
use crate::input::{KeyRepeat, Rumble};

/// One pad's frame in the ring (`cPS2Device` vf8: 0x2c bytes): the 16 raw bytes (+0), whether the pad answered
/// (+0x10), whether it is in analog mode (+0x14), the libpad buttons inverted so pressed = 1 (+0x18, a halfword),
/// and the four stick bytes (+0x1c left x, +0x20 left y, +0x24 right x, +0x28 right y, words).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub raw: [u8; 16],
    pub valid: u32,
    pub analog: u32,
    pub buttons: u16,
    pub sticks: [u32; 4],
}

pub const RECORD_SIZE: usize = 0x2c;

impl Record {
    pub fn to_bytes(&self) -> [u8; RECORD_SIZE] {
        let mut b = [0u8; RECORD_SIZE];
        b[..16].copy_from_slice(&self.raw);
        b[0x10..0x14].copy_from_slice(&self.valid.to_le_bytes());
        b[0x14..0x18].copy_from_slice(&self.analog.to_le_bytes());
        b[0x18..0x1a].copy_from_slice(&self.buttons.to_le_bytes());
        for (i, s) in self.sticks.iter().enumerate() {
            b[0x1c + 4 * i..0x20 + 4 * i].copy_from_slice(&s.to_le_bytes());
        }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Record {
        let w = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let mut raw = [0u8; 16];
        raw.copy_from_slice(&b[..16]);
        Record { raw, valid: w(0x10), analog: w(0x14), buttons: u16::from_le_bytes([b[0x18], b[0x19]]), sticks: [w(0x1c), w(0x20), w(0x24), w(0x28)] }
    }

    /// vf9's record when the pad answered (state 10 and `scePadRead` gave data): `pad` is libpad's 20-byte read
    /// (+2 and +3 the buttons inverted, +4..+7 the sticks right x, right y, left x, left y, +8.. the pressure
    /// bytes); `analog` is the device's mode (+0x50), `pressure` whether it has pressure sensitive buttons (+0x4c)
    /// and gives the 12 pressure bytes their places in `raw`.
    pub fn from_pad(pad: &[u8; 20], analog: u32, pressure: bool) -> Record {
        let mut r = Record { valid: 1, analog, buttons: !u16::from_be_bytes([pad[2], pad[3]]), ..Record::default() };
        r.sticks = [pad[6] as u32, pad[7] as u32, pad[4] as u32, pad[5] as u32];
        if pressure {
            r.raw[0] = pad[10];
            r.raw[1] = pad[11];
            r.raw[2] = pad[8];
            r.raw[3] = pad[9];
            r.raw[4] = pad[15];
            r.raw[5] = pad[12];
            r.raw[6] = pad[13];
            r.raw[9] = pad[16];
            r.raw[10] = pad[17];
            r.raw[11] = pad[14];
            r.raw[12] = pad[18];
            r.raw[13] = pad[19];
        }
        r
    }

    /// vf9's record when it has no pad data: all zero, and valid only when the device has seen the pad go away
    /// (`+0x44`, a pad that was there and is not).
    pub fn none(gone: bool) -> Record {
        Record { valid: gone as u32, ..Record::default() }
    }
}

/// A pad device as the list holds it (`cPS2Device` and the `cPadDevice` under it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps2Pad {
    /// +4: the last frame had a pad in it
    pub valid: u32,
    /// +0x68: the raw bytes of the last frame (the pressure of each button)
    pub raw: [u8; 16],
    /// +0xc .. +0x28
    pub dev: PadDevice,
    /// +0x4c: the pad has pressure sensitive buttons (set by the poll)
    pub pressure: u32,
    /// +0x2c .. +0x34
    pub repeat: KeyRepeat,
    pub rumble: Rumble,
}

impl Default for Ps2Pad {
    fn default() -> Self {
        Ps2Pad { valid: 0, raw: [0; 16], dev: PadDevice::default(), pressure: 0, repeat: KeyRepeat::default(), rumble: Rumble::default() }
    }
}

impl Ps2Pad {
    /// vf10 0x17b278: take a frame. A frame with no pad leaves everything but `valid` as it was.
    pub fn play(&mut self, r: &Record) {
        self.valid = r.valid;
        if r.valid == 0 {
            return;
        }
        self.raw = r.raw;
        let analog = (r.analog != 0).then(|| [r.sticks[0] as u8, r.sticks[1] as u8, r.sticks[2] as u8, r.sticks[3] as u8]);
        self.dev.read(r.buttons, analog);
        self.repeat.update(self.dev.buttons);
    }

    /// vf1 0x17b610: whether the pad can do feature `n` (it has pressure, `n` below 14, not 7 or 8)
    pub fn supports(&self, n: u32) -> bool {
        self.pressure != 0 && n < 0xe && n != 7 && n != 8
    }

    /// vf2 0x17b640: the pressure of raw byte `i`, 0..1
    pub fn pressure_of(&self, i: usize) -> f32 {
        self.raw[i] as f32 / 255.0
    }

    /// `Pad_LatchButtons` 0x17a658: this tick's buttons become last tick's.
    pub fn latch(&mut self) {
        self.dev.prev = self.dev.buttons;
    }

    /// the "new buttons" of `Input_GetAnyNewButtons`: down now and not before
    pub fn new_buttons(&self) -> u32 {
        self.dev.buttons & !self.dev.prev
    }

    /// `Input_ResetDevices` for one pad: the small motor off, the large motor off (both until told: −1 frames)
    pub fn reset(&mut self) {
        self.rumble.set_small(0, -1);
        self.rumble.set_large(0.0, -1);
    }
}

/// `Input_FindPressedDevice` 0x17a840: the first device whose key this frame (`+0x2c`) is not 0, with its index.
pub fn find_pressed(pads: &[Ps2Pad]) -> Option<(u32, usize)> {
    pads.iter().enumerate().find(|(_, p)| p.repeat.key != 0).map(|(i, p)| (p.repeat.key, i))
}

/// `Input_GetAnyNewButtons` 0x1834a8
pub fn any_new_buttons(pads: &[Ps2Pad]) -> u32 {
    pads.iter().fold(0, |a, p| a | p.new_buttons())
}

/// `Input_ResetDevices` 0x17a8b0
pub fn reset_devices(pads: &mut [Ps2Pad]) {
    for p in pads {
        p.reset();
    }
}

/// The ring (the list's +0 .. +0x10): `frame` bytes a frame, `cap` frames.
#[derive(Clone, Debug)]
pub struct Ring {
    pub buf: Vec<u8>,
    pub frame: usize,
    pub cap: usize,
    pub read: usize,
    pub write: usize,
}

impl Ring {
    /// `Input_Init` 0x17a420 for `devices` records of `sizes` bytes (the two pads, 0x2c each): eight frames.
    pub fn new(sizes: &[usize]) -> Ring {
        let frame = sizes.iter().sum::<usize>();
        let cap = 8;
        Ring { buf: vec![0; frame * cap], frame, cap, read: 0, write: 0 }
    }

    /// `Input_ReadDevices` 0x17a668: the devices' frames (`records`, one per device in order) are written at the
    /// write index, which then steps.
    pub fn write_frame(&mut self, records: &[Vec<u8>]) {
        let mut at = self.write * self.frame;
        for r in records {
            self.buf[at..at + r.len()].copy_from_slice(r);
            at += r.len();
        }
        self.write = (self.write + 1) % self.cap;
    }

    /// `InputRecorder_Playback` 0x17a740: the frame at the read index, split by the devices' `sizes`, or `None`
    /// when the ring is empty (read = write); the read index steps when there was one.
    pub fn read_frame(&mut self, sizes: &[usize]) -> Option<Vec<Vec<u8>>> {
        if self.read == self.write {
            return None;
        }
        let mut at = self.read * self.frame;
        let mut out = Vec::new();
        for s in sizes {
            out.push(self.buf[at..at + s].to_vec());
            at += s;
        }
        self.read = (self.read + 1) % self.cap;
        Some(out)
    }
}

/// The list as the game keeps it: the two pads and their ring.
#[derive(Clone, Debug)]
pub struct Input {
    pub pads: Vec<Ps2Pad>,
    pub ring: Ring,
}

impl Input {
    /// `Input_Init`: pads on ports 0 and 1
    pub fn new() -> Input {
        Input { pads: vec![Ps2Pad::default(); 2], ring: Ring::new(&[RECORD_SIZE, RECORD_SIZE]) }
    }

    /// `Input_ReadDevices` with this tick's `records` (what each pad's poll gave)
    pub fn read_devices(&mut self, records: &[Record]) {
        let b: Vec<Vec<u8>> = records.iter().map(|r| r.to_bytes().to_vec()).collect();
        self.ring.write_frame(&b);
    }

    /// `InputRecorder_Playback`: the next frame goes to the pads; false when the ring is empty.
    pub fn playback(&mut self) -> bool {
        let sizes = vec![RECORD_SIZE; self.pads.len()];
        match self.ring.read_frame(&sizes) {
            None => false,
            Some(fs) => {
                for (p, f) in self.pads.iter_mut().zip(fs) {
                    p.play(&Record::from_bytes(&f));
                }
                true
            }
        }
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(n: u8) -> Record {
        Record { raw: [n; 16], valid: 1, analog: 1, buttons: 0x0100 + n as u16, sticks: [n as u32, 127, 128, 255] }
    }

    #[test]
    fn record_bytes_round_trip() {
        let r = rec(7);
        assert_eq!(Record::from_bytes(&r.to_bytes()), r);
        assert_eq!(r.to_bytes().len(), 0x2c);
    }

    #[test]
    fn ring_is_empty_when_read_meets_write() {
        let mut i = Input::new();
        assert!(!i.playback());
        i.read_devices(&[rec(1), rec(2)]);
        assert!(i.playback());
        assert!(!i.playback());
        assert_eq!((i.ring.read, i.ring.write), (1, 1));
    }

    #[test]
    fn ring_wraps_at_eight_and_has_no_full_check() {
        let mut i = Input::new();
        for n in 0..8 {
            i.read_devices(&[rec(n), rec(n)]);
        }
        // eight writes caught the read index: it looks empty
        assert_eq!(i.ring.write, 0);
        assert!(!i.playback());
        // seven frames fit
        let mut i = Input::new();
        for n in 0..7 {
            i.read_devices(&[rec(n), rec(100 + n)]);
        }
        for n in 0..7 {
            assert!(i.playback());
            assert_eq!(i.pads[0].raw[0], n);
            assert_eq!(i.pads[1].raw[0], 100 + n);
        }
        assert!(!i.playback());
    }

    #[test]
    fn a_frame_with_no_pad_changes_only_valid() {
        let mut p = Ps2Pad::default();
        p.play(&rec(3));
        let before = p;
        p.play(&Record::none(false));
        assert_eq!(p.valid, 0);
        assert_eq!(Ps2Pad { valid: 1, ..p }, before);
    }

    #[test]
    fn pad_record_from_libpad() {
        let mut pad = [0u8; 20];
        pad[2] = 0xff;
        pad[3] = 0xfe; // inverted: bit 0 pressed
        pad[4..8].copy_from_slice(&[10, 20, 30, 40]);
        for (i, b) in pad[8..].iter_mut().enumerate() {
            *b = 100 + i as u8;
        }
        let r = Record::from_pad(&pad, 1, true);
        assert_eq!(r.buttons, 1);
        assert_eq!(r.sticks, [30, 40, 10, 20]);
        assert_eq!(r.raw[0], 102);
        assert_eq!(r.raw[7], 0);
        let r = Record::from_pad(&pad, 0, false);
        assert_eq!(r.raw, [0; 16]);
        assert_eq!(r.analog, 0);
        assert_eq!(Record::none(true).valid, 1);
    }

    #[test]
    fn pressed_and_new_buttons() {
        let mut i = Input::new();
        assert_eq!(find_pressed(&i.pads), None);
        i.read_devices(&[rec(0), Record { buttons: 0x0100, valid: 1, ..Record::default() }]);
        assert!(i.playback());
        assert!(any_new_buttons(&i.pads) != 0);
        for p in &mut i.pads {
            p.latch();
        }
        assert_eq!(any_new_buttons(&i.pads), 0);
        assert_eq!(find_pressed(&i.pads).map(|x| x.1).unwrap_or(9) < 2, true);
    }

    #[test]
    fn reset_turns_the_motors_off() {
        let mut p = [Ps2Pad::default(), Ps2Pad::default()];
        p[1].rumble.set_small(1, 30);
        p[1].rumble.set_large(1.0, 30);
        reset_devices(&mut p);
        assert_eq!((p[1].rumble.small, p[1].rumble.large, p[1].rumble.small_frames, p[1].rumble.large_frames), (0, 0, -1, -1));
    }
}

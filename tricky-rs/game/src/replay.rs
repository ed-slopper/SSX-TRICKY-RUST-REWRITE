//! The replay's byte streams and pad streams (board row G12a), 1:1 with the game's `Replay_*` / `ReplayInput_*`
//! (0x1926f0..0x193068), checked bit for bit in the runner (`tests/replay.rs`).
//!
//! A replay keeps each keyframe's snapshot of the world in a byte stream (`Stream`: the buffer, the write and read
//! positions, the bytes written) and each rider's pad as a run-length list (`Input`). The node references a
//! snapshot writes are instance numbers (the node's address less the course's instance array, over 0x100) and the
//! offsets are from the course's data base (0x34649c).

/// The replay control's words these read and write (gApp+0x730 → +0x40)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Control {
    /// +0xc: the size of a keyframe's stream
    pub stream_size: i32,
    /// +0x20: the mode (0 recording; 1, 2, 4 playing)
    pub mode: u32,
    /// +0x3c / +0x40: the pad streams full (a 180-frame message)
    pub full_timer: u32,
    pub input_full: u32,
    /// +0x4c, +0x50: recording held
    pub x4c: u32,
    pub x50: u32,
    /// +0x58: a stream overflowed
    pub overflow: u32,
}

/// `EAMem_Set`'s fill for a new stream ("Frame Block", 0x38a378): the word 0xdeadc0ed over and over
pub const FILL: u32 = 0xdead_c0ed;

/// `EAMem_Set` 0x2cc808(dst at `addr`, value, size): a byte, a half word and a word to align `addr` to 8, then
/// 64-byte blocks, 16-byte blocks, words, a half word and a byte; each store takes the value's low byte, half or
/// word, so a pattern wider than a byte depends on where the stores fall
pub fn ea_mem_set(dst: &mut [u8], addr: u32, value: u32) {
    let v = value.to_le_bytes();
    let (mut at, mut n, mut a) = (0usize, dst.len() as i32, addr);
    let mut put = |at: &mut usize, n: &mut i32, a: &mut u32, w: usize| {
        dst[*at..*at + w].copy_from_slice(&v[..w]);
        *at += w;
        *n -= w as i32;
        *a = a.wrapping_add(w as u32);
    };
    if n > 0 && a & 1 != 0 {
        put(&mut at, &mut n, &mut a, 1);
    }
    if n >= 2 && a & 2 != 0 {
        put(&mut at, &mut n, &mut a, 2);
    }
    if n >= 4 && a & 4 != 0 {
        put(&mut at, &mut n, &mut a, 4);
    }
    while n >= 4 {
        put(&mut at, &mut n, &mut a, 4);
    }
    if n >= 2 {
        put(&mut at, &mut n, &mut a, 2);
    }
    if n > 0 {
        put(&mut at, &mut n, &mut a, 1);
    }
}

/// A byte stream (16 bytes in the game: the buffer, the write and read pointers, the bytes written)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stream {
    pub buf: Vec<u8>,
    pub write: usize,
    pub read: usize,
    pub written: i32,
}

impl Stream {
    /// `Replay_AllocStream` 0x192b38(size): the buffer ("Frame Block", `EAMem_Alloc` aligns it) filled with 0xdeadc0ed,
    /// both positions at its start
    pub fn alloc(size: usize) -> Stream {
        let mut buf = vec![0; size];
        ea_mem_set(&mut buf, 0, FILL);
        Stream { buf, write: 0, read: 0, written: 0 }
    }

    /// `Replay_FreeStream` 0x192ba8
    pub fn free(&mut self) {
        *self = Stream::default();
    }

    /// `Replay_RewindRead` 0x192bf0
    pub fn rewind_read(&mut self) {
        self.read = 0;
    }

    /// `Replay_ResetWrite` 0x192c00
    pub fn reset_write(&mut self) {
        self.written = 0;
        self.write = 0;
    }

    /// `Replay_Write` 0x192c10: nothing once a stream has overflowed; bytes that do not fit in the control's stream
    /// size (+0xc less what is written) mark the overflow (+0x58) instead
    pub fn write(&mut self, ctl: &mut Control, src: &[u8]) {
        if ctl.overflow != 0 {
            return;
        }
        let n = src.len() as i32;
        if ctl.stream_size.wrapping_sub(self.write as i32) < n {
            ctl.overflow = 1;
            return;
        }
        let end = self.write + src.len();
        assert!(end <= self.buf.len(), "Replay_Write: the game writes past the stream's buffer");
        self.buf[self.write..end].copy_from_slice(src);
        self.write = end;
        self.written = self.written.wrapping_add(n);
    }

    /// `Replay_Read` 0x192ca0: no check against what was written
    pub fn read(&mut self, dst: &mut [u8]) {
        let end = self.read + dst.len();
        assert!(end <= self.buf.len(), "Replay_Read: the game reads past the stream's buffer");
        dst.copy_from_slice(&self.buf[self.read..end]);
        self.read = end;
    }

    pub fn read_i32(&mut self) -> i32 {
        let mut b = [0; 4];
        self.read(&mut b);
        i32::from_le_bytes(b)
    }

    /// `Replay_Peek` 0x192ce8: a read that leaves the read position
    pub fn peek(&mut self, dst: &mut [u8]) {
        let at = self.read;
        self.read(dst);
        self.read = at;
    }

    /// `Replay_PeekNodeType` 0x192d20: the third word of the next node's 12-byte header
    pub fn peek_node_type(&mut self) -> u32 {
        let mut b = [0; 12];
        self.peek(&mut b);
        u32::from_le_bytes([b[8], b[9], b[10], b[11]])
    }

    /// `Replay_UnpackBlock` 0x192d60(the source's cursor): a packed block (its size, then `Decompress_Block`'s
    /// input) into the buffer, the write position at its unpacked end, the read position at the start. (It also
    /// copies 4 bytes from its third argument to a stack word nothing reads.)
    pub fn unpack_block(&mut self, src: &[u8], cursor: &mut usize, decompress: &mut dyn FnMut(&[u8], &mut [u8]) -> usize) -> bool {
        let size = i32::from_le_bytes(src[*cursor..*cursor + 4].try_into().unwrap());
        *cursor += 4;
        let n = decompress(&src[*cursor..], &mut self.buf);
        *cursor = (*cursor as i32).wrapping_add(size) as usize;
        self.write = n;
        self.rewind_read();
        true
    }
}

/// What a snapshot's references are relative to: the course (*0x34648c) +0x48 its instances (0x100 bytes each),
/// +0xc their count; the data base at 0x34649c
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Refs {
    pub instances: u32,
    pub count: i32,
    pub data: u32,
}

impl Refs {
    /// `Course_GetInstance` 0x254a28 on the course: an instance's address, 0 past the count
    pub fn instance(&self, index: i32) -> u32 {
        if index < self.count {
            self.instances.wrapping_add((index as u32) << 8)
        } else {
            0
        }
    }

    fn index(&self, node: u32) -> i32 {
        (node.wrapping_sub(self.instances) as i32) >> 8
    }
}

/// `Replay_WriteNodeRef` 0x192f30(node): its instance number, −1 for none
pub fn write_node_ref(s: &mut Stream, ctl: &mut Control, refs: &Refs, node: u32) {
    let v = if node != 0 { refs.index(node) } else { -1 };
    s.write(ctl, &v.to_le_bytes());
}

/// `Replay_ReadNodeRef` 0x192df0: the instance, 0 for none
pub fn read_node_ref(s: &mut Stream, refs: &Refs) -> u32 {
    let v = s.read_i32();
    if v < 0 { 0 } else { refs.instance(v) }
}

/// `Replay_WriteLink` 0x192f78(owner, node; the node's +0xe4 and the low half of +0xe8): the instance number, the
/// half word and whether the node's +0xe4 is the owner; −1 and zeros for none
pub fn write_link(s: &mut Stream, ctl: &mut Control, refs: &Refs, owner: u32, node: u32, node_e4: u32, node_e8: u16) {
    let (i, word, flag) = if node == 0 { (-1, 0u16, 0u16) } else { (refs.index(node), node_e8, (node_e4 == owner) as u16) };
    let mut b = [0u8; 8];
    b[..4].copy_from_slice(&i.to_le_bytes());
    b[4..6].copy_from_slice(&word.to_le_bytes());
    b[6..].copy_from_slice(&flag.to_le_bytes());
    s.write(ctl, &b);
}

/// `Replay_ReadLink` 0x192e40(owner): the instance; with the flag set, its +0xe4 becomes the owner and the low half
/// of +0xe8 the saved half word (returned for the caller to set)
pub fn read_link(s: &mut Stream, refs: &Refs) -> (u32, Option<u16>) {
    let mut b = [0u8; 8];
    s.read(&mut b);
    let i = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    if i < 0 {
        return (0, None);
    }
    let node = refs.instance(i);
    let flag = u16::from_le_bytes([b[6], b[7]]);
    if node == 0 || flag == 0 {
        return (node, None);
    }
    (node, Some(u16::from_le_bytes([b[4], b[5]])))
}

/// `Replay_WriteOffset` 0x193020 / `Replay_WriteOffset2` 0x192fe8 (the same code): an address less the data base
pub fn write_offset(s: &mut Stream, ctl: &mut Control, refs: &Refs, addr: u32) {
    s.write(ctl, &addr.wrapping_sub(refs.data).to_le_bytes());
}

/// `Replay_ReadOffset` 0x192ec0 / `Replay_ReadOffset2` 0x192ef8
pub fn read_offset(s: &mut Stream, refs: &Refs) -> u32 {
    refs.data.wrapping_add(s.read_i32() as u32)
}

/// The most pad words a stream holds (0x1999; 0x6664 bytes of them)
pub const INPUT_WORDS: usize = 0x1999;

/// A rider's pad stream (0x800c bytes in the game): the pad words, each one's run of frames (a byte at +0x6664),
/// the count (+0x8000), and in playback the word (+0x8004) and the frames left of its run (+0x8008)
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub words: Vec<u32>,
    /// 0x199c bytes: up to +0x8000
    pub runs: Vec<u8>,
    pub count: i32,
    pub pos: i32,
    pub left: i32,
}

impl Default for Input {
    fn default() -> Input {
        Input { words: vec![0; INPUT_WORDS], runs: vec![0; 0x8000 - 0x6664], count: 0, pos: 0, left: 0 }
    }
}

impl Input {
    /// `ReplayInput_Reset` 0x192738 (and its copy 0x192750); `ReplayInput_Init` 0x1926f0 is the reset
    pub fn reset(&mut self) {
        self.left = 0;
        self.count = 0;
        self.pos = 0;
    }

    /// `ReplayInput_SeekFrame` 0x192758(frame): the word playing at that frame and the frames left of its run;
    /// false (nothing changed) past the end
    pub fn seek_frame(&mut self, frame: u32) -> bool {
        if self.count <= 0 {
            return false;
        }
        let mut i = 0i32;
        let mut end = self.runs[0] as u32;
        while end <= frame {
            i += 1;
            if self.count <= i {
                return false;
            }
            end = end.wrapping_add(self.runs[i as usize] as u32);
        }
        self.pos = i;
        self.left = end.wrapping_sub(frame) as i32;
        true
    }

    /// `Replay_PushInput` 0x192880: a new word with a run of 1; past 0x1998 words the control's pad streams full
    /// (+0x3c 180, +0x40 1) instead
    pub fn push(&mut self, ctl: &mut Control, word: u32) {
        if self.count > 0x1998 {
            ctl.full_timer = 180;
            ctl.input_full = 1;
            return;
        }
        let n = self.count as usize;
        self.runs[n] = 1;
        self.words[n] = word;
        self.count += 1;
    }

    /// `Replay_RecordInput` 0x1928f0: the last word's run one longer when the pad is the same (and the run under
    /// 255), else a new word
    pub fn record(&mut self, ctl: &mut Control, word: u32) -> bool {
        if self.count < 1 {
            self.push(ctl, word);
            return true;
        }
        let last = (self.count - 1) as usize;
        if self.words[last] == word && self.runs[last] != 0xff {
            self.runs[last] += 1;
            return true;
        }
        self.push(ctl, word);
        true
    }

    /// `Replay_ReadInput` 0x192998: the next frame's word (0 and false past the end)
    pub fn read(&mut self) -> (bool, u32) {
        let i = self.pos;
        if i >= self.count {
            return (false, 0);
        }
        if self.left < 1 {
            self.left = self.runs[i as usize] as i32;
        }
        let w = self.words[i as usize];
        self.left -= 1;
        if self.left < 1 {
            self.pos += 1;
        }
        (true, w)
    }

    /// `ReplayInput_Load` 0x192a40(the source's cursor): an 8-byte header (its second word the count), then two
    /// packed blocks (a size, then `Decompress_Block`'s input): the words, the runs
    pub fn load(&mut self, src: &[u8], cursor: &mut usize, decompress: &mut dyn FnMut(&[u8], &mut [u8]) -> usize) -> bool {
        let word = |at: usize| i32::from_le_bytes(src[at..at + 4].try_into().unwrap());
        self.count = word(*cursor + 4);
        *cursor += 8;
        let size = word(*cursor);
        *cursor += 4;
        let mut bytes = vec![0u8; INPUT_WORDS * 4];
        for (k, w) in self.words.iter().enumerate() {
            bytes[4 * k..4 * k + 4].copy_from_slice(&w.to_le_bytes());
        }
        decompress(&src[*cursor..], &mut bytes);
        for (k, w) in self.words.iter_mut().enumerate() {
            *w = u32::from_le_bytes(bytes[4 * k..4 * k + 4].try_into().unwrap());
        }
        *cursor = (*cursor as i32).wrapping_add(size) as usize;
        let size = word(*cursor);
        *cursor += 4;
        decompress(&src[*cursor..], &mut self.runs);
        *cursor = (*cursor as i32).wrapping_add(size) as usize;
        true
    }
}

/// `Replay_RecordOrPlayInput` 0x1927d8(stream, pad): recording (mode 0, not held by +0x50 / +0x4c / +0x40) records
/// the pad and answers true; playing (1, 2, 4) reads it into the pad; else false
pub fn record_or_play(ctl: &mut Control, input: &mut Input, pad: &mut u32) -> bool {
    match ctl.mode {
        0 => {
            if ctl.x50 != 0 || ctl.x4c != 0 || ctl.input_full != 0 {
                return false;
            }
            input.record(ctl, *pad);
            true
        }
        1 | 2 | 4 => {
            let (ok, w) = input.read();
            *pad = w;
            ok
        }
        _ => false,
    }
}

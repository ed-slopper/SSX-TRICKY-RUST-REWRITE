//! EA's MicroTalk speech codec (the sound library's codec 4), as the game decodes it (board row E1b2g7d3b2). The
//! announcer's and the riders' speech files (`.dat`, EA `SCHl` streams) are MicroTalk: 432-sample frames of a
//! twelve-pole LPC filter driven by a pitch predictor and a Huffman-coded excitation.
//!
//! [`Mt`] is the decoder's state as the game lays it out (0x10 into its decoder record: the bit reader, the
//! threshold, 64 gains, the reflection coefficients, the filter's history and the 756-float excitation buffer, of
//! which the last 432 are the frame). [`Channel`] is one channel of a stream (`SndMt_PullStream` 0x2e17d0) and
//! [`decode`] a whole `SCHl` part.

use crate::ps2f::F;

/// The bit reader (`SndBits_Read` 0x2e2b98): bits come out lowest first from a byte buffer that is topped up one
/// byte at a time whenever fewer than eight are left.
#[derive(Clone, Copy, Default)]
pub struct Bits {
    pub pos: usize,
    pub buf: u32,
    pub count: u32,
}

impl Bits {
    /// A reader at `data[pos]` (`SndMt_ParseHeader` and `SndMt_PullStream` set it up so).
    pub fn at(data: &[u8], pos: usize) -> Bits {
        Bits { pos: pos + 1, buf: byte(data, pos), count: 8 }
    }

    /// `SndBits_Read` 0x2e2b98
    pub fn read(&mut self, data: &[u8], n: u32) -> u32 {
        let v = self.buf & ((1u32 << n) - 1);
        self.skip(data, n);
        v
    }

    /// `SndBits_Skip` 0x2e2c00
    pub fn skip(&mut self, data: &[u8], n: u32) {
        self.buf >>= n;
        self.count -= n;
        if self.count < 8 {
            self.buf |= byte(data, self.pos) << self.count;
            self.pos += 1;
            self.count += 8;
        }
    }
}

// STANDIN: a read past the data gives 0; the game reads whatever follows the block in memory (only the last
// frame's padding bits can reach it).
fn byte(data: &[u8], pos: usize) -> u32 {
    data.get(pos).copied().unwrap_or(0) as u32
}

/// The reflection coefficients the frame header's indices pick (0x35a738).
const RC: [u32; 64] = [
    0x00000000, 0xbf7f2cb6, 0xbf7d8612, 0xbf7bdf7f, 0xbf7a38eb, 0xbf789247, 0xbf76ebb3, 0xbf75450f,
    0xbf739e7c, 0xbf6e45e5, 0xbf67ab75, 0xbf611117, 0xbf5a76a7, 0xbf53dc38, 0xbf4d41d9, 0xbf46a769,
    0xbf400cfa, 0xbf39729b, 0xbf32d82c, 0xbf2baebc, 0xbf1e79ee, 0xbf11450f, 0xbf041041, 0xbeedb6e5,
    0xbed34d27, 0xbeb8e38a, 0xbe9e79ee, 0xbe841052, 0xbe534d27, 0xbe1e79ee, 0xbdd34d6a, 0xbd534ce4,
    0x00000000, 0x3d534ce4, 0x3dd34d6a, 0x3e1e79ee, 0x3e534d27, 0x3e841052, 0x3e9e79ee, 0x3eb8e38a,
    0x3ed34d27, 0x3eedb6e5, 0x3f041041, 0x3f11450f, 0x3f1e79ee, 0x3f2baebc, 0x3f32d82c, 0x3f39729b,
    0x3f400cfa, 0x3f46a769, 0x3f4d41d9, 0x3f53dc38, 0x3f5a76a7, 0x3f611117, 0x3f67ab75, 0x3f6e45e5,
    0x3f739e7c, 0x3f75450f, 0x3f76ebb3, 0x3f789247, 0x3f7a38eb, 0x3f7bdf7f, 0x3f7d8612, 0x3f7f2cb6,
];

/// The excitation's Huffman lookup: two models of 256, by the next eight bits (0x35a838).
const HUFF: [[u8; 256]; 2] = [
    [
        4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 17, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 21,
        4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 18, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 25,
        4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 17, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 22,
        4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 18, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 0,
        4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 17, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 21,
        4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 18, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 26,
        4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 17, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 22,
        4, 6, 5, 9, 4, 6, 5, 13, 4, 6, 5, 10, 4, 6, 5, 18, 4, 6, 5, 9, 4, 6, 5, 14, 4, 6, 5, 10, 4, 6, 5, 2,
    ],
    [
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 23, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 27,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 24, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 1,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 23, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 28,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 24, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 3,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 23, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 27,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 24, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 1,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 23, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 28,
        4, 11, 7, 15, 4, 12, 8, 19, 4, 11, 7, 16, 4, 12, 8, 24, 4, 11, 7, 15, 4, 12, 8, 20, 4, 11, 7, 16, 4, 12, 8, 3,
    ],
];

/// Each Huffman symbol: (next model, bits it takes, value) (0x35aa38).
const SYMBOLS: [(usize, u32, f32); 29] = [
    (1, 8, 0.0), (1, 7, 0.0), (0, 8, 0.0), (0, 7, 0.0), (0, 2, 0.0), (0, 2, -1.0),
    (0, 2, 1.0), (0, 3, -1.0), (0, 3, 1.0), (1, 4, -2.0), (1, 4, 2.0), (1, 3, -2.0),
    (1, 3, 2.0), (1, 5, -3.0), (1, 5, 3.0), (1, 4, -3.0), (1, 4, 3.0), (1, 6, -4.0),
    (1, 6, 4.0), (1, 5, -4.0), (1, 5, 4.0), (1, 7, -5.0), (1, 7, 5.0), (1, 6, -5.0),
    (1, 6, 5.0), (1, 8, -6.0), (1, 8, 6.0), (1, 7, -6.0), (1, 7, 6.0),
];

// The EE's float ops (no denormals: a filter dying away reaches them).
fn add(a: f32, b: f32) -> f32 {
    (F(a) + F(b)).0
}
fn sub(a: f32, b: f32) -> f32 {
    (F(a) - F(b)).0
}
fn mul(a: f32, b: f32) -> f32 {
    (F(a) * F(b)).0
}

pub const FRAME: usize = 432;
const SUB: usize = 108;
const HISTORY: usize = 324;

/// The decoder's state (decoder record +0x10).
#[derive(Clone)]
pub struct Mt {
    /// +0x0
    pub bits: Bits,
    /// +0xc: frames whose first coefficient index is below it use the Huffman excitation
    pub threshold: i32,
    /// +0x10
    pub gains: [f32; 64],
    /// +0x110: the reflection coefficients
    pub rc: [f32; 12],
    /// +0x140: the synthesis filter's last twelve outputs
    pub hist: [f32; 12],
    /// +0x170: 324 floats of past excitation, then the frame (432)
    pub a: [f32; HISTORY + FRAME],
}

impl Default for Mt {
    fn default() -> Mt {
        Mt { bits: Bits::default(), threshold: 0, gains: [0.0; 64], rc: [0.0; 12], hist: [0.0; 12], a: [0.0; HISTORY + FRAME] }
    }
}

impl Mt {
    /// The decoded frame (after [`Mt::decode_frame`]).
    pub fn frame(&self) -> &[f32] {
        &self.a[HISTORY..]
    }

    /// `SndMt_ParseHeader` 0x2e3070: the stream's header bits at `data[pos]`; resets the filter and the excitation.
    pub fn parse_header(&mut self, data: &[u8], pos: usize) {
        self.bits = Bits::at(data, pos);
        self.bits.read(data, 1);
        self.threshold = 32 - self.bits.read(data, 4) as i32;
        self.gains[0] = mul((self.bits.read(data, 4) + 1) as f32, 8.0);
        let ratio = add(mul(self.bits.read(data, 6) as f32, f32::from_bits(0x3a83126f)), f32::from_bits(0x3f851eb8));
        for i in 1..64 {
            self.gains[i] = mul(self.gains[i - 1], ratio);
        }
        self.rc = [0.0; 12];
        self.hist = [0.0; 12];
        self.a[..HISTORY].fill(0.0);
    }

    /// `SndMt_DecodeFrame` 0x2e31e8: one frame of 432 samples into [`Mt::frame`].
    pub fn decode_frame(&mut self, data: &[u8]) {
        let rc = |i: u32| f32::from_bits(RC[i as usize]);
        let mut delta = [0.0f32; 12];
        let first = self.bits.read(data, 6);
        let huffman = (first as i32) < self.threshold;
        delta[0] = mul(sub(rc(first), self.rc[0]), 0.25);
        for (k, (d, old)) in delta.iter_mut().zip(self.rc).enumerate().skip(1) {
            let i = if k < 4 { self.bits.read(data, 6) } else { self.bits.read(data, 5) + 16 };
            *d = mul(sub(rc(i), old), 0.25);
        }
        for s in 0..4 {
            let lag = self.bits.read(data, 8) as usize;
            let pitch = mul(self.bits.read(data, 4) as f32, f32::from_bits(0x3d888889));
            let mut gain = self.gains[self.bits.read(data, 6) as usize];
            let odd = self.bits.read(data, 1) as usize;
            let plain = self.bits.read(data, 1);
            let mut ex = [0.0f32; 5 + SUB + 5];
            read_excitation(&mut self.bits, data, huffman, &mut ex[5 + odd..]);
            if plain == 0 {
                ex[..5].fill(0.0);
                ex[5 + SUB..].fill(0.0);
                interpolate_odd(&mut ex, 6 - odd);
                gain = mul(gain, 0.5);
            } else {
                for k in 0..54 {
                    ex[6 - odd + 2 * k] = 0.0;
                }
            }
            for j in 0..SUB {
                let past = self.a[(s + 2) * SUB - lag + j];
                self.a[HISTORY + s * SUB + j] = add(mul(pitch, past), mul(gain, ex[5 + j]));
            }
        }
        self.a.copy_within(FRAME..FRAME + HISTORY, 0);
        for (at, blocks) in [(0, 1), (12, 1), (24, 1), (36, 33)] {
            for (r, d) in self.rc.iter_mut().zip(delta) {
                *r = add(*r, d);
            }
            self.synth_subframe(at, blocks);
        }
    }

    /// `SndMt_SynthSubframe` 0x2e3010: the coefficients to a filter, run over `blocks` twelves of the frame from
    /// `at`.
    fn synth_subframe(&mut self, at: usize, blocks: usize) {
        let lpc = rc_to_lpc(&self.rc);
        lpc_synth(blocks, &lpc, &mut self.hist, &mut self.a[HISTORY + at..]);
    }
}

/// `SndMt_ReadExcitation` 0x2e2c50: 54 values into every second float of `out`. Without `huffman`, two bits
/// each (01 → −2, 11 → 2, else one bit, 0); with it, symbols of two Huffman models, escapes for larger values
/// and runs of zeros.
fn read_excitation(bits: &mut Bits, data: &[u8], huffman: bool, out: &mut [f32]) {
    if !huffman {
        for k in 0..54 {
            out[2 * k] = match bits.buf & 3 {
                1 => {
                    bits.skip(data, 2);
                    -2.0
                }
                3 => {
                    bits.skip(data, 2);
                    2.0
                }
                _ => {
                    bits.skip(data, 1);
                    0.0
                }
            };
        }
        return;
    }
    let (mut model, mut i) = (0, 0usize);
    loop {
        let sym = HUFF[model][(bits.buf & 0xff) as usize] as usize;
        let (next, len, value) = SYMBOLS[sym];
        model = next;
        bits.skip(data, len);
        if sym < 4 {
            if sym < 2 {
                let mut n = 7;
                while bits.read(data, 1) == 1 {
                    n += 1;
                }
                out[i] = if bits.read(data, 1) != 1 { -(n as f32) } else { n as f32 };
                i += 2;
            } else {
                let mut n = bits.read(data, 6) as usize + 7;
                if i + 2 * n > SUB {
                    n = (SUB - i) >> 1;
                }
                for k in 0..n {
                    out[i + 2 * k] = 0.0;
                }
                i += 2 * n;
            }
        } else {
            out[i] = value;
            i += 2;
        }
        if i > SUB - 1 {
            return;
        }
    }
}

/// `SndMt_InterpolateOdd` 0x2e2e98: the 54 values between the read ones, from `ex[at]` in steps of two.
fn interpolate_odd(ex: &mut [f32], at: usize) {
    let (c0, c1, c2) = (f32::from_bits(0x3f18ee49), f32::from_bits(0xbdeaaefb), f32::from_bits(0x3c93b945));
    for k in 0..54 {
        let p = at + 2 * k;
        let near = mul(add(ex[p - 1], ex[p + 1]), c0);
        let mid = mul(add(ex[p - 3], ex[p + 3]), c1);
        ex[p] = add(add(near, mid), mul(add(ex[p - 5], ex[p + 5]), c2));
    }
}

/// `SndMt_RcToLpc` 0x2e2f10: reflection coefficients to the direct-form filter.
fn rc_to_lpc(rc: &[f32; 12]) -> [f32; 12] {
    let mut b = [0.0f32; 12];
    b[1..].copy_from_slice(&rc[..11]);
    b[0] = 1.0;
    let (mut t, mut lpc) = ([0.0f32; 12], [0.0f32; 12]);
    for n in 0..12 {
        let mut f = mul(-rc[11], b[11]);
        for k in (0..11).rev() {
            f = sub(f, mul(rc[k], b[k]));
            b[k + 1] = add(mul(rc[k], f), b[k]);
        }
        b[0] = f;
        t[n] = f;
        for j in 0..n {
            f = sub(f, mul(lpc[j], t[n - 1 - j]));
        }
        lpc[n] = f;
    }
    lpc
}

/// `SndMt_LpcSynth` 0x2e3620 (hand-written, on the FPU's accumulator): the all-pole filter over `blocks` twelves of
/// `x`, in place. `hist` is a ring of the last twelve outputs; each output adds, to its input, `lpc[k]` times the
/// output `k + 1` back, the newest last except in a block's first sample.
fn lpc_synth(blocks: usize, lpc: &[f32; 12], hist: &mut [f32; 12], x: &mut [f32]) {
    for b in 0..blocks {
        for t in 0..12 {
            let at = 12 * b + t;
            let term = |k: usize| mul(lpc[k], hist[(k + 12 - t) % 12]);
            let mut acc = add(x[at], 0.0);
            let order = if t == 0 { [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] } else { [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0] };
            for k in order {
                acc = add(acc, term(k));
            }
            let y = add(acc, mul(0.0, 0.0));
            hist[11 - t] = y;
            x[at] = y;
        }
    }
}

/// One channel of a MicroTalk stream (`SndMt_PullStream` 0x2e17d0): each data block starts with a byte, 0 to go
/// on from the last block's bits and frame, else the stream's header follows (a fresh decoder).
#[derive(Clone, Default)]
pub struct Channel {
    pub mt: Box<Mt>,
    /// samples of the current frame not given out yet
    pub left: usize,
}

impl Channel {
    /// The channel's `n` samples of one block `data` onto `out`.
    pub fn block(&mut self, data: &[u8], n: usize, out: &mut Vec<f32>) {
        if byte(data, 0) == 0 {
            self.mt.bits = Bits::at(data, 1);
        } else {
            self.mt.parse_header(data, 1);
            self.left = 0;
        }
        let mut n = n;
        while n > 0 {
            if self.left == 0 {
                self.mt.decode_frame(data);
                self.left = FRAME;
            }
            let take = n.min(self.left);
            out.extend_from_slice(&self.mt.frame()[FRAME - self.left..FRAME - self.left + take]);
            self.left -= take;
            n -= take;
        }
    }
}

/// `Snd_FloatToPcm16` 0x2e3a5c (from `SndFx_PcmOut_Pull` 0x2e39d0): clamped, then cut toward 0.
pub fn pcm16(x: f32) -> i16 {
    x.clamp(-32768.0, 32767.0) as i32 as i16
}

/// One decoded `SCHl` part.
pub struct Sound {
    pub rate: u32,
    pub channels: usize,
    /// interleaved
    pub samples: Vec<i16>,
}

/// A part's header: its rate, channels and codec as `SndSound_ParseHeader` 0x2d60b0 reads the PT tags (only the
/// rate 0x84, channels 0x82 and codec 0xa0; defaults 22050, 1, 5; the tags as `SndPT_NextTag` 0x2dffe8 walks
/// them), and where its first chunk after the header starts.
pub struct Header {
    pub rate: u32,
    pub channels: usize,
    pub codec: u32,
    pub end: usize,
}

/// The `SCHl` chunk at `data[at]`.
pub fn header(data: &[u8], at: usize) -> Option<Header> {
    if data.get(at..at + 4)? != b"SCHl" {
        return None;
    }
    let (mut rate, mut channels, mut codec) = (22050u32, 1usize, 5u32);
    let end = at + u32::from_le_bytes(data.get(at + 4..at + 8)?.try_into().ok()?) as usize;
    let mut p = at + 12; // "PT" and two bytes
    while p < end {
        match data[p] {
            0xff => break,
            0xfc..=0xfe => p += 1,
            tag => {
                let mut size = *data.get(p + 1)? as usize;
                p += 2;
                if size == 0xff {
                    size = u32::from_be_bytes(data.get(p..p + 4)?.try_into().ok()?) as usize;
                    p += 4;
                }
                let value = data.get(p..p + size.min(4))?.iter().fold(0u32, |v, &b| v << 8 | b as u32);
                match tag {
                    0x84 => rate = value,
                    0x82 => channels = value as usize,
                    0xa0 => codec = value,
                    _ => {}
                }
                p += size;
            }
        }
    }
    Some(Header { rate, channels, codec, end })
}

/// The `SCHl` part at `data[at]` decoded, if it is MicroTalk: the data blocks as `SndStream_OnDataChunk` 0x2d1e88
/// splits them (samples, then each channel's offset), each channel through its [`Channel`].
// STANDIN: our walk over the part's chunks, for the stream reader's (`SndStream_*`, 0x2d1b70..0x2d2948).
pub fn decode(data: &[u8], at: usize) -> Option<Sound> {
    let word = |p: usize| data.get(p..p + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let Header { rate, channels, codec, end } = header(data, at)?;
    if codec != 4 || channels == 0 {
        return None;
    }
    let mut chans = vec![Channel::default(); channels];
    let mut out: Vec<Vec<f32>> = vec![Vec::new(); channels];
    let mut p = end;
    while let Some(size) = word(p + 4) {
        match data.get(p..p + 4)? {
            b"SCDl" => {
                let n = (word(p + 8)? & 0x7fff_ffff) as usize;
                for (c, ch) in chans.iter_mut().enumerate() {
                    let start = p + 12 + 4 * channels + word(p + 12 + 4 * c)? as usize;
                    ch.block(data.get(start..p + size as usize)?, n, &mut out[c]);
                }
            }
            b"SCEl" => break,
            _ => {}
        }
        if size < 8 {
            break;
        }
        p += size as usize;
    }
    let len = out.iter().map(Vec::len).min().unwrap_or(0);
    let samples = (0..len).flat_map(|i| out.iter().map(move |c| pcm16(c[i]))).collect();
    Some(Sound { rate, channels, samples })
}

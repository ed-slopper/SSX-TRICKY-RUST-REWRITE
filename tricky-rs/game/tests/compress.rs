//! The codecs the disc does not use (board row E2c) against the game in the function runner (TRICKY_ELF; skipped
//! without it): streams made here for each type (Huffman with runs, escapes and every header form, the long
//! numbers' two paths included; byte pairs; 4×4 colour blocks), unpacked by the game's `Compress_Decode`
//! 0x2c2de0 and by `tricky_data::compress`, byte for byte.

use r5900::Runner;
use tricky_data::compress::{
    btree_decode, bytes_read_be, decode18, huff_block_is_compressed, huff_block_unpacked_size, huff_decode, huff_decode_block,
};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

/// Big-endian bits, most significant first.
#[derive(Default)]
struct BitW {
    bytes: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitW {
    fn put(&mut self, v: u32, n: u32) {
        for i in (0..n).rev() {
            self.acc = self.acc << 1 | (v >> i & 1) as u64;
            self.n += 1;
            if self.n == 8 {
                self.bytes.push(self.acc as u8);
                self.acc = 0;
                self.n = 0;
            }
        }
    }
    /// A number as the game reads it (`HuffBits::number`): z zeros, a one, n bits, with n = z + 2 below 16
    /// zeros and z + 3 from there.
    fn num(&mut self, v: u32) {
        let n = 31 - v.leading_zeros();
        if n == 2 {
            self.put(1, 1);
            self.put(v - 4, 2);
            return;
        }
        let z = if n <= 17 { n - 2 } else { n - 3 };
        assert!(z != 15 || n == 17, "a number the game cannot read");
        self.put(0, z);
        self.put(1, 1);
        self.put(v - (1 << n), n);
    }
    fn finish(mut self) -> Vec<u8> {
        while self.n != 0 {
            self.put(0, 1);
        }
        self.bytes
    }
}

/// Code lengths for `freq` (Huffman, by merging the two least weights), at most 16.
fn lengths(freq: &[(u8, u64)]) -> Vec<(u8, u32)> {
    let mut nodes: Vec<(u64, Vec<usize>)> = freq.iter().enumerate().map(|(i, f)| (f.1.max(1), vec![i])).collect();
    let mut len = vec![0u32; freq.len()];
    while nodes.len() > 1 {
        nodes.sort_by(|a, b| b.0.cmp(&a.0));
        let a = nodes.pop().unwrap();
        let b = nodes.pop().unwrap();
        for &i in a.1.iter().chain(&b.1) {
            len[i] += 1;
        }
        nodes.push((a.0 + b.0, [a.1, b.1].concat()));
    }
    freq.iter().zip(len).map(|(f, l)| (f.0, l)).collect()
}

/// A Huffman stream of type `ty` for `data`; `runs` / `escapes` say how often to use the run and literal forms.
fn huff_encode(g: &mut Rng, ty: u8, data: &[u8], runs: bool, escapes: u32) -> Vec<u8> {
    let mut freq = [0u64; 256];
    for &b in data {
        freq[b as usize] += 1;
    }
    let clue = g.below(256) as u8;
    let mut syms: Vec<(u8, u64)> = (0..=255u8).filter(|&b| freq[b as usize] > 0 && b != clue).map(|b| (b, freq[b as usize])).collect();
    syms.push((clue, 1 + g.below(50) as u64));
    if syms.len() == 1 {
        syms.push((clue ^ 1, 1));
    }
    let mut lens = lengths(&syms);
    if lens.iter().any(|l| l.1 > 16) {
        // flatten: give every symbol 8 bits plus the escape
        lens = (0..=255u8).map(|b| (b, 8)).collect();
    }
    lens.sort_by_key(|&(b, l)| (l, b));
    let max = lens.iter().map(|l| l.1).max().unwrap();
    let mut code = vec![(0u32, 0u32); 256];
    let mut c = 0u32;
    let mut i = 0;
    let mut counts = vec![0u32; max as usize + 1];
    for len in 1..=max {
        while i < lens.len() && lens[i].1 == len {
            code[lens[i].0 as usize] = (c, len);
            c += 1;
            counts[len as usize] += 1;
            i += 1;
        }
        c <<= 1;
    }
    let mut w = BitW::default();
    let size = data.len() as u32;
    let mut head = vec![ty, 0xfb];
    if ty & 0x80 != 0 {
        if ty & 1 != 0 {
            head.extend([0xde, 0xad, 0xbe, 0xef]);
        }
        head.extend(size.to_be_bytes());
    } else {
        if ty & 1 != 0 {
            head.extend([0x12, 0x34, 0x56]);
        }
        w.put(size, 24);
    }
    w.put(clue as u32, 8);
    for &n in &counts[1..] {
        w.num(n + 4);
    }
    let mut used = [false; 256];
    let mut last = 0xffu32;
    for &(b, _) in &lens {
        let mut k = 0;
        loop {
            last = (last + 1) & 0xff;
            if !used[last as usize] {
                k += 1;
            }
            if last == b as u32 {
                break;
            }
        }
        used[b as usize] = true;
        w.num(k + 3);
    }
    let emit = |w: &mut BitW, b: u8| {
        let (c, l) = code[b as usize];
        w.put(c, l);
    };
    let mut p = 0;
    while p < data.len() {
        let b = data[p];
        if b == clue || (escapes > 0 && g.below(escapes) == 0) {
            emit(&mut w, clue);
            w.num(4);
            w.put(0, 1);
            w.put(b as u32, 8);
        } else {
            emit(&mut w, b);
        }
        p += 1;
        if runs && p > 0 {
            let mut r = 0;
            while p + r < data.len() && data[p + r] == b {
                r += 1;
            }
            // a run of 2^18..2^19 cannot be written: leave the rest as plain codes
            let r = if (1 << 18..1 << 19).contains(&(r as u32 + 4)) { (1 << 18) - 5 } else { r };
            if r >= 2 {
                emit(&mut w, clue);
                w.num(r as u32 + 4);
                p += r;
            }
        }
    }
    emit(&mut w, clue);
    w.num(4);
    w.put(1, 1);
    let mut out = head;
    out.extend(w.finish());
    out.extend([0u8; 16]);
    out
}

fn sample(g: &mut Rng, n: usize) -> Vec<u8> {
    let kind = g.below(4);
    let mut v = vec![];
    while v.len() < n {
        let b = match kind {
            0 => g.below(256) as u8,
            1 => (g.below(16) * g.below(16) / 15) as u8,
            2 => b"the quick brown fox jumps over the lazy dog "[g.below(44) as usize],
            _ => [0u8, 0, 0, 1, 2, 0xff, 0x80][g.below(7) as usize],
        };
        let rep = if g.below(6) == 0 { 1 + g.below(40) as usize } else { 1 };
        v.extend(std::iter::repeat_n(b, rep));
    }
    v.truncate(n);
    v
}

struct Game {
    r: Runner,
    src: u32,
    dst: u32,
}

fn runner() -> Option<Game> {
    let mut r = Runner::from_env()?;
    for a in [0x2c3730, 0x2c3540, 0x2c34d8, 0x2c2d18, 0x2c2bb8, 0x2c2bf8, 0x2c27c8, 0x2c31b0] {
        r.allow(a);
    }
    let src = r.mem.alloc(0x4_0000);
    let dst = r.mem.alloc(0x10_0000);
    Some(Game { r, src, dst })
}

/// `Compress_Decode(src, dst)` in the runner: its result and `n` bytes of the buffer.
fn game(g: &mut Game, src: &[u8], n: usize) -> (u32, Vec<u8>) {
    #[allow(non_snake_case)]
    let (r, SRC, DST) = (&mut g.r, g.src, g.dst);
    assert!(src.len() <= 0x4_0000 && n + 64 <= 0x10_0000);
    r.mem.write(SRC, src).unwrap();
    r.mem.write(DST, &vec![0u8; n + 64]).unwrap();
    r.cpu.set_gpr(4, SRC as u64);
    r.cpu.set_gpr(5, DST as u64);
    r.call(0x2c2de0).unwrap();
    let mut out = vec![0u8; n];
    r.mem.read(DST, &mut out).unwrap();
    (r.cpu.gpr(2) as u32, out)
}

#[test]
fn huffman_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x4875_6666_0000_0001);
    let types = [0x30u8, 0x31, 0x32, 0x33, 0x34, 0x35, 0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5];
    let mut bytes = 0;
    for case in 0..1500 {
        let ty = types[case % types.len()];
        let n = [0usize, 1, 2, 7, 100, 1000, 5000][g.below(7) as usize];
        let data = sample(&mut g, n);
        let (runs, esc) = (g.below(3) != 0, [0, 0, 3, 50][g.below(4) as usize]);
        let src = huff_encode(&mut g, ty, &data, runs, esc);
        let mut ours = vec![];
        let size = huff_decode(&src, &mut ours);
        assert_eq!(size as usize, data.len(), "case {case}");
        if ty & 0xfe == 0x30 || ty & 0x80 != 0 {
            assert_eq!(ours, data, "case {case}: our decode of our own stream");
        }
        let (ret, theirs) = game(&mut r, &src, ours.len().max(size as usize));
        assert_eq!(ret, size, "case {case}: type {ty:#x}");
        assert_eq!(theirs, ours, "case {case}: type {ty:#x}, {n} bytes");
        bytes += ours.len();
    }
    // the long numbers: runs of 2^17.. (17 bits, read in two halves) and 2^19.. (19+ zeros: counted one by one)
    for (case, run) in [131_072usize, 200_000, 262_139, 530_000, 700_000].into_iter().enumerate() {
        let mut data = sample(&mut g, 50);
        data.extend(std::iter::repeat_n(7u8, run));
        data.extend(sample(&mut g, 50));
        let src = huff_encode(&mut g, 0x30, &data, true, 0);
        let mut ours = vec![];
        huff_decode(&src, &mut ours);
        assert_eq!(ours, data, "long {case}");
        let (_, theirs) = game(&mut r, &src, ours.len());
        assert!(theirs == ours, "long run {run}: the game differs");
        bytes += ours.len();
    }
    eprintln!("{bytes} bytes compared");
}

/// `Huff_DecodeBlock` 0x240ef8(src, dst) in the runner: its result and `n` bytes of the buffer
fn game_block(g: &mut Game, src: &[u8], n: usize) -> (u32, Vec<u8>) {
    #[allow(non_snake_case)]
    let (r, SRC, DST) = (&mut g.r, g.src, g.dst);
    r.mem.write(SRC, src).unwrap();
    r.mem.write(DST, &vec![0u8; n + 64]).unwrap();
    r.cpu.set_gpr(4, SRC as u64);
    r.cpu.set_gpr(5, 0);
    r.cpu.set_gpr(6, DST as u64);
    // through Decompress_Block 0x241e28(src, 0, dst), as the replay calls it
    r.call(0x241e28).unwrap();
    let mut out = vec![0u8; n];
    r.mem.read(DST, &mut out).unwrap();
    (r.cpu.gpr(2) as u32, out)
}

/// Row G12b: the replay's older copy of the decoder, on the same streams; the types with bit 7 encoded without
/// the 4-byte size (the older copy has none), so they decode as their plain types but never sum
#[test]
fn huffman_block_matches_the_original() {
    let Some(mut r) = runner() else { return };
    r.r.allow(0x240ef8);
    let mut g = Rng(0x4875_6666_0000_0b0c);
    let mut bytes = 0;
    for case in 0..1500 {
        let ty = [0x30u8, 0x31, 0x32, 0x33, 0x34, 0x35][case % 6];
        let n = [0usize, 1, 2, 7, 100, 1000, 5000][g.below(7) as usize];
        let data = sample(&mut g, n);
        let (runs, esc) = (g.below(3) != 0, [0, 0, 3, 50][g.below(4) as usize]);
        let mut src = huff_encode(&mut g, ty, &data, runs, esc);
        if case % 12 >= 6 {
            src[0] |= 0x80;
        }
        let mut ours = vec![];
        let size = huff_decode_block(&src, &mut ours);
        assert_eq!(size as usize, data.len(), "case {case}");
        if src[0] & 0xfe == 0x30 || src[0] & 0x80 != 0 {
            assert_eq!(ours, data, "case {case}: our decode of our own stream");
        }
        let (ret, theirs) = game_block(&mut r, &src, ours.len().max(size as usize));
        assert_eq!(ret, size, "case {case}: type {:#x}", src[0]);
        assert_eq!(theirs, ours, "case {case}: type {:#x}, {n} bytes", src[0]);
        bytes += ours.len();
    }
    for (case, run) in [131_072usize, 200_000, 262_139, 530_000, 700_000].into_iter().enumerate() {
        let mut data = sample(&mut g, 50);
        data.extend(std::iter::repeat_n(7u8, run));
        data.extend(sample(&mut g, 50));
        let src = huff_encode(&mut g, 0x30, &data, true, 0);
        let mut ours = vec![];
        huff_decode_block(&src, &mut ours);
        assert_eq!(ours, data, "long {case}");
        let (_, theirs) = game_block(&mut r, &src, ours.len());
        assert!(theirs == ours, "long run {run}: the game differs");
        bytes += ours.len();
    }
    // its header helpers: Bytes_ReadBE 0x241cf0, Huff_IsCompressed 0x241d28, Huff_GetUnpackedSize 0x241dd8
    r.r.allow(0x241cf0);
    for case in 0..20_000 {
        let mut b: Vec<u8> = (0..8).map(|_| g.next() as u8).collect();
        if g.below(2) == 0 {
            b[0] = 0x2f + g.below(8) as u8;
            b[1] = if g.below(4) == 0 { g.next() as u8 } else { 0xfb };
        }
        r.r.mem.write(r.src, &b).unwrap();
        let k = g.below(7) as usize;
        let mut call = |f: u32, a1: u64| {
            r.r.cpu.set_gpr(4, r.src as u64);
            r.r.cpu.set_gpr(5, a1);
            r.r.call(f).unwrap();
            r.r.cpu.gpr(2) as u32
        };
        assert_eq!(call(0x241cf0, k as u64), bytes_read_be(&b, k), "case {case}: read {k}");
        assert_eq!(call(0x241d28, 0), huff_block_is_compressed(&b), "case {case}: {b:x?}");
        assert_eq!(call(0x241dd8, 0), huff_block_unpacked_size(&b), "case {case}: {b:x?}");
    }
    eprintln!("{bytes} bytes compared");
}

#[test]
fn btree_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0xb7ee_0000_0000_0003);
    for case in 0..3000 {
        let clue = g.below(256) as u8;
        let mut nodes: Vec<u8> = vec![];
        let count = g.below(60);
        while nodes.len() < count as usize {
            let b = g.below(256) as u8;
            if b != clue && !nodes.contains(&b) {
                nodes.push(b);
            }
        }
        let marked = |c: u8| c == clue || nodes.contains(&c);
        // halves: plain bytes or pairs defined before (no loops), never the escape byte
        let mut defs = vec![];
        for (i, &b) in nodes.iter().enumerate() {
            let half = |g: &mut Rng| loop {
                let c = if i > 0 && g.below(3) == 0 { nodes[g.below(i as u32) as usize] } else { g.below(256) as u8 };
                if nodes[..i].contains(&c) || !marked(c) {
                    break c;
                }
            };
            let (l, rr) = (half(&mut g), half(&mut g));
            defs.push([b, l, rr]);
        }
        let mut body = vec![];
        for _ in 0..g.below(400) {
            match g.below(4) {
                0 if !nodes.is_empty() => body.push(nodes[g.below(nodes.len() as u32) as usize]),
                1 => {
                    body.push(clue);
                    body.push(1 + g.below(255) as u8);
                }
                _ => {
                    let c = g.below(256) as u8;
                    if !marked(c) {
                        body.push(c);
                    }
                }
            }
        }
        body.push(clue);
        body.push(0);
        let ty: u16 = [0x46fb, 0x47fb, 0xc6fb, 0xc7fb][g.below(4) as usize];
        let mut src = ty.to_be_bytes().to_vec();
        if ty == 0x47fb {
            src.extend([9, 9, 9]);
        }
        let size = g.below(5000);
        src.extend(&size.to_be_bytes()[1..]);
        src.push(clue);
        src.push(defs.len() as u8);
        for d in &defs {
            src.extend(d);
        }
        src.extend(&body);
        src.extend([0u8; 8]);
        let mut ours = vec![];
        let ret = btree_decode(&src, &mut ours);
        let (theirs_ret, theirs) = game(&mut r, &src, ours.len());
        assert_eq!((theirs_ret, &theirs), (ret, &ours), "case {case}");
    }
}

#[test]
fn colour_blocks_match_the_original() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0xdc7_0000_0000_0005);
    for case in 0..3000 {
        let ty = [0x18u8, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x98, 0x9a, 0x9c][g.below(9) as usize];
        let width = [1u32, 2, 4, 6, 8, 12, 16, 32, 64, 10][g.below(10) as usize];
        let size = width * (1 + g.below(40)) * 2 + [0, 0, 0, 1, 7][g.below(5) as usize];
        let mut src = vec![ty, 0xfb];
        src.extend(&size.to_be_bytes()[1..]);
        src.push(g.below(256) as u8);
        src.extend((width as u16).to_be_bytes());
        let blocks = (size / (width * 8) + 1) * width.div_ceil(4) * 2 + 4;
        for _ in 0..blocks {
            let pick = |g: &mut Rng| match g.below(5) {
                0 => 0u16,
                1 => 0xffff,
                _ => g.next() as u16,
            };
            let (c0, c1) = (pick(&mut g), pick(&mut g));
            src.extend(c0.to_le_bytes());
            src.extend(c1.to_le_bytes());
            for _ in 0..4 {
                src.push(g.below(256) as u8);
            }
        }
        let mut ours = vec![];
        let ret = decode18(&src, &mut ours);
        assert_eq!(ret, size);
        let (theirs_ret, theirs) = game(&mut r, &src, ours.len());
        assert_eq!(theirs_ret, ret, "case {case}");
        assert!(theirs == ours, "case {case}: type {ty:#x}, width {width}, size {size}: first difference at {:?}", theirs.iter().zip(&ours).position(|(a, b)| a != b));
    }
}

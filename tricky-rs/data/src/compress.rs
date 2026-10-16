//! EA's compressed files (board row E2b): `Compress_UnpackedSize` 0x2c2fb0, `Compress_Decode` 0x2c2de0 and
//! `RefPack_Decode` 0x2c31b0, as the game's file loader (0x2c8938) uses them.
//!
//! A packed file starts with a type byte and 0xfb. `Compress_Decode` hands the type (low bit cleared) to one
//! of EA's codecs: RefPack (0x10, 0x90), Huffman (0x30, 0x32, 0x34, 0xb0, 0xb2, 0xb4), BTree (0x46, 0xc6),
//! a 4×4-block colour codec at 0x2c2d18 (0x18, 0x1a, 0x1c, 0x98, 0x9a, 0x9c) and a registered one (0x1e, 0x9e,
//! never registered). Every packed file on the disc is RefPack (0x10/0x11); the other three are row E2c, for mods
//! that pack files with them.

/// `Compress_UnpackedSize` 0x2c2fb0: the size a packed file unpacks to, or 0 when it is not packed (or uses
/// the registered codec, which the game never registers). Types with bit 7 keep a 4-byte size at +2, the
/// others a 3-byte one, read at +2 whatever the low bit says.
pub fn unpacked_size(src: &[u8]) -> u32 {
    if src.len() < 2 || src[1] != 0xfb {
        return 0;
    }
    let width = match src[0] & 0xfe {
        0x10 | 0x18 | 0x1a | 0x30 | 0x32 | 0x34 | 0x46 => 3,
        0x90 | 0x98 | 0x9a | 0xb0 | 0xb2 | 0xb4 | 0xc6 => 4,
        // 0x1e / 0x9e: the registered codec (gp-0x7fc8), null in the game
        _ => return 0,
    };
    src.get(2..2 + width).map_or(0, |b| b.iter().fold(0u32, |v, &c| v << 8 | c as u32))
}

/// `Compress_Decode` 0x2c2de0 with an output buffer of `Compress_UnpackedSize` bytes, as the loader 0x2c8938
/// does: the unpacked bytes, or `None` when the type is not one the game decodes. (The codecs write what the
/// stream says; the buffer keeps its first `Compress_UnpackedSize` bytes, zeros past what was written.)
pub fn decode(src: &[u8]) -> Option<Vec<u8>> {
    if src.len() < 2 || src[1] != 0xfb {
        return None;
    }
    let mut out = Vec::with_capacity(unpacked_size(src) as usize);
    match src[0] & 0xfe {
        0x10 | 0x90 => {
            refpack_decode(src, &mut out);
        }
        0x30 | 0x32 | 0x34 | 0xb0 | 0xb2 | 0xb4 => {
            huff_decode(src, &mut out);
        }
        0x46 | 0xc6 => {
            btree_decode(src, &mut out);
        }
        0x18 | 0x1a | 0x1c | 0x98 | 0x9a | 0x9c => {
            decode18(src, &mut out);
        }
        _ => return None,
    }
    // the loader's buffer is `Compress_UnpackedSize` bytes
    out.resize(unpacked_size(src) as usize, 0);
    Some(out)
}

/// The loader's rule (0x2c8938): a file with a non-zero `Compress_UnpackedSize` is decoded, any other file is
/// used as read.
pub fn load(data: Vec<u8>) -> Option<Vec<u8>> {
    if unpacked_size(&data) == 0 {
        return Some(data);
    }
    decode(&data)
}

/// `RefPack_Decode` 0x2c31b0. The header is a big-endian u16 type; with bit 0x8000 the size is 4 bytes,
/// otherwise 3, and with bit 0x100 one more size field comes first (skipped). Then commands until one of
/// 0xfc-0xff:
/// - `0ooLLLpp oooooooo`: `pp` literals, then `LLL`+3 bytes from `ooooooooooo`+1 back;
/// - `10LLLLLL ppoooooo oooooooo`: `pp` literals, `LLLLLL`+4 bytes from 14 bits of offset +1 back;
/// - `110oLLpp oooooooo oooooooo LLLLLLLL`: `pp` literals, `LL`·256 + byte + 5 bytes from 17 bits +1 back;
/// - `111nnnnn`: (`nnnnn`+1)·4 literals;
/// - `111111pp`: `pp` literals and the end.
///
/// Copies are byte by byte, so a copy may overlap what it writes. Returns the size field, as the game does.
pub fn refpack_decode(src: &[u8], out: &mut Vec<u8>) -> u32 {
    let ty = (src[0] as u32) << 8 | src[1] as u32;
    let (at, width) = match (ty & 0x8000 != 0, ty & 0x100 != 0) {
        (false, false) => (2, 3),
        (false, true) => (5, 3),
        (true, false) => (2, 4),
        (true, true) => (6, 4),
    };
    let size = src[at..at + width].iter().fold(0u32, |v, &c| v << 8 | c as u32);
    let mut p = at + width;
    let copy = |out: &mut Vec<u8>, back: usize, n: usize| {
        let from = out.len() - back;
        for i in 0..n {
            out.push(out[from + i]);
        }
    };
    loop {
        let c = src[p] as usize;
        p += 1;
        if c & 0x80 == 0 {
            let b = src[p] as usize;
            let lit = c & 3;
            out.extend_from_slice(&src[p + 1..p + 1 + lit]);
            p += 1 + lit;
            copy(out, (b | (c & 0x60) << 3) + 1, ((c & 0x1c) >> 2) + 3);
        } else if c & 0x40 == 0 {
            let (b1, b2) = (src[p] as usize, src[p + 1] as usize);
            let lit = b1 >> 6;
            out.extend_from_slice(&src[p + 2..p + 2 + lit]);
            p += 2 + lit;
            copy(out, (b2 | (b1 & 0x3f) << 8) + 1, (c & 0x3f) + 4);
        } else if c & 0x20 == 0 {
            let (b1, b2, b3) = (src[p] as usize, src[p + 1] as usize, src[p + 2] as usize);
            let lit = c & 3;
            out.extend_from_slice(&src[p + 3..p + 3 + lit]);
            p += 3 + lit;
            let hi = if c & 0x10 != 0 { 0x10000 } else { 0 };
            copy(out, (b2 | (hi + (b1 << 8))) + 1, (c & 0xc) * 0x40 + b3 + 5);
        } else {
            let n = (c & 0x1f) * 4;
            if n + 4 > 0x70 {
                let lit = c & 3;
                out.extend_from_slice(&src[p..p + lit]);
                return size;
            }
            out.extend_from_slice(&src[p..p + n + 4]);
            p += n + 4;
        }
    }
}

fn be(src: &[u8], at: usize, n: usize) -> u32 {
    (0..n).fold(0u32, |v, k| v << 8 | *src.get(at + k).unwrap_or(&0) as u32)
}

/// `Huff_Decode` 0x2c3730's bit reader: `acc` the last bytes read (16 at a time, big-endian), `win` the next
/// bits at the top of a word, `bits` how many valid bits the window has past 16.
struct HuffBits<'a> {
    src: &'a [u8],
    p: usize,
    acc: u32,
    win: u32,
    bits: i32,
}

impl HuffBits<'_> {
    /// Drop `n` bits from the window `w`, reading two more bytes when they run out.
    fn take(&mut self, w: u32, n: u32) {
        self.bits -= n as i32;
        self.win = w.wrapping_shl(n);
        if self.bits < 0 {
            self.refill();
            self.win = self.acc.wrapping_shl((-self.bits) as u32);
            self.bits += 16;
        }
    }

    fn refill(&mut self) {
        self.acc = self.acc << 16 | be(self.src, self.p, 2);
        self.p += 2;
    }

    /// The codec's numbers: `1ab` is 4 + ab; else z zeros, a one, then n = z + 2 bits, giving the bits + 2^n.
    /// With 16 or more zeros (the window's top half empty) the game counts the zeros one bit at a time and ends
    /// up with n = z + 3 instead.
    fn number(&mut self) -> u32 {
        let w = self.win;
        if (w as i32) < 0 {
            self.take(w, 3);
            return w >> 29;
        }
        let n = if w >> 16 == 0 {
            let mut n = 2;
            loop {
                n += 1;
                let top = self.win >> 31;
                self.take(self.win, 1);
                if top != 0 {
                    break n;
                }
            }
        } else {
            let z = w.leading_zeros();
            self.take(w, z + 1);
            2 + z
        };
        let u = self.win;
        if n < 17 {
            self.take(u, n);
            u.wrapping_shr(32 - n).wrapping_add(1u32.wrapping_shl(n))
        } else {
            let hi = u.wrapping_shr(48u32.wrapping_sub(n));
            self.take(u, n - 16);
            let lo = self.win;
            self.take(lo, 16);
            (lo >> 16 | hi << 16).wrapping_add(1u32.wrapping_shl(n))
        }
    }
}

/// `Huff_Decode` 0x2c3730 (types 0x30, 0x32, 0x34 and with bit 7 or bit 0): EA's Huffman codec with runs.
/// Appends to `out`; returns the size in the header, as the game does.
///
/// The header: with bit 7 a 4-byte size at +2 (with bit 0 at +6, after a skipped 4-byte field) and the bit
/// stream after it; without, the bit stream starts at +2 (with bit 0 at +5) and its first 24 bits are the size.
/// Then: the escape byte (8 bits); for each code length from 1, a number − 4 codes of that length, until the
/// codes fill the tree; for each code (in canonical order) a number − 3: how many not yet used byte values to
/// step forward from the last one (from 0xff, wrapping) to its byte. Then codes; the escape code is followed by
/// a number − 4: when not 0 the last byte repeated that many times, else a bit: 0 an 8-bit literal, 1 the end.
/// Codes up to 8 bits come from a 256-entry table, longer ones by comparing the window's top 16 bits with each
/// length's last code (from length 9). Type 0x32fb then sums the bytes (each the sum of those up to it), 0x34fb
/// twice; the types with bit 7 are compared whole, so 0xb2 and 0xb4 skip this.
pub fn huff_decode(src: &[u8], out: &mut Vec<u8>) -> u32 {
    huff_decode_with(src, out, true)
}

/// The older copy of the decoder the replays use (`Huff_DecodeBlock` 0x240ef8, through `Decompress_Block`
/// 0x241e28; board row G12b): the same code without the 4-byte size, so bit 7 of the type only keeps 0xb2 / 0xb4
/// from summing.
pub fn huff_decode_block(src: &[u8], out: &mut Vec<u8>) -> u32 {
    huff_decode_with(src, out, false)
}

/// `Bytes_ReadBE` 0x241cf0(bytes, n): n bytes big-endian (wrapping past 4)
pub fn bytes_read_be(src: &[u8], n: usize) -> u32 {
    src[..n].iter().fold(0u32, |v, &c| v.wrapping_mul(0x100).wrapping_add(c as u32))
}

/// `Huff_IsCompressed` 0x241d28, beside `Huff_DecodeBlock`: 100 for the types 0x30fb..0x35fb, else 0
pub fn huff_block_is_compressed(src: &[u8]) -> u32 {
    if (0x30fb..=0x35fb).step_by(0x100).any(|t| bytes_read_be(src, 2) == t) { 100 } else { 0 }
}

/// `Huff_GetUnpackedSize` 0x241dd8: the 3-byte size at +2, or at +5 when the type's bit 0x100 is set
pub fn huff_block_unpacked_size(src: &[u8]) -> u32 {
    if bytes_read_be(src, 2) & 0x100 == 0 { bytes_read_be(&src[2..], 3) } else { bytes_read_be(&src[5..], 3) }
}

fn huff_decode_with(src: &[u8], out: &mut Vec<u8>, long_size: bool) -> u32 {
    let start = out.len();
    let b0 = src[0];
    let mut r = HuffBits { src, p: 4, acc: be(src, 2, 2), win: 0, bits: 0 };
    r.win = r.acc << 16;
    let size;
    if b0 & 0x80 == 0 || !long_size {
        if b0 & 1 != 0 {
            r.bits = 8;
            r.p = 8;
            r.acc = be(src, 5, 3);
            r.win = r.acc << 8;
        }
        let hi = r.win;
        r.take(hi, 8);
        let lo = r.win;
        r.take(lo, 16);
        size = lo >> 16 | (hi >> 24) << 16;
    } else {
        let mut hi = r.acc << 16;
        if b0 & 1 != 0 {
            r.p = 8;
            hi = be(src, 6, 2) << 16;
        }
        size = be(src, r.p, 2) | hi;
        r.acc = be(src, r.p + 2, 2);
        r.win = r.acc << 16;
        r.p += 4;
        r.bits = 0;
    }
    let ty = (b0 as u32) << 8 | src[1] as u32;
    let ty = ty & 0xfeff;

    // the tables
    let clue = r.win >> 24;
    r.take(r.win, 8);
    let (mut base, mut counts, mut cmp) = (vec![0i32; 2], vec![0i32; 2], vec![0u32; 2]);
    let (mut total, mut code) = (0i32, 0i32);
    let mut level = 1usize;
    loop {
        if base.len() <= level + 1 {
            base.resize(level + 2, 0);
            counts.resize(level + 2, 0);
            cmp.resize(level + 2, 0);
        }
        base[level] = code.wrapping_mul(2).wrapping_sub(total);
        let n = r.number().wrapping_sub(4) as i32;
        total = total.wrapping_add(n);
        counts[level] = n;
        code = code.wrapping_mul(2).wrapping_add(n);
        let c = if n != 0 { (code as u32).wrapping_shl((16i32 - level as i32) as u32) & 0xffff } else { 0 };
        cmp[level] = c;
        if n != 0 && c == 0 {
            break;
        }
        level += 1;
    }
    let max_len = level;
    cmp[max_len] = 0xffff_ffff;
    let mut used = [false; 256];
    let mut leaves = vec![0u8; total.max(0) as usize];
    let mut last = 0xffu32;
    for leaf in leaves.iter_mut() {
        let mut k = r.number().wrapping_sub(3) as i32;
        let mut u;
        loop {
            u = last + 1;
            last = u & 0xff;
            if !used[last as usize] {
                k -= 1;
            }
            if k == 0 {
                break;
            }
        }
        used[last as usize] = true;
        *leaf = u as u8;
    }
    let mut lens = [0x40u8; 256];
    let mut syms = [0u8; 256];
    let mut clue_len = 0u32;
    let (mut li, mut at) = (0usize, 0usize);
    for len in 1..=max_len.min(8) {
        for _ in 0..counts[len].max(0) {
            let Some(&sym) = leaves.get(li) else { break };
            li += 1;
            let l = if sym as u32 == clue {
                clue_len = len as u32;
                0x60
            } else {
                len as u8
            };
            for _ in 0..1 << (8 - len) {
                if at < 256 {
                    syms[at] = sym;
                    lens[at] = l;
                }
                at += 1;
            }
        }
    }

    // the codes
    loop {
        let mut len = lens[(r.win >> 24) as usize] as u32;
        r.bits -= len as i32;
        while r.bits >= 0 {
            out.push(syms[(r.win >> 24) as usize]);
            r.win = r.win.wrapping_shl(len);
            len = lens[(r.win >> 24) as usize] as u32;
            r.bits -= len as i32;
        }
        r.bits += 16;
        if r.bits >= 0 {
            out.push(syms[(r.win >> 24) as usize]);
            r.refill();
            r.win = r.acc.wrapping_shl((16 - r.bits) as u32);
            continue;
        }
        let n = if len != 0x60 {
            let mut n = 9;
            while cmp.get(n).copied().unwrap_or(0xffff_ffff) <= r.win >> 16 {
                n += 1;
            }
            n as u32
        } else {
            clue_len
        };
        r.bits = r.bits + (len as i32 - 16) - n as i32;
        let c = r.win.wrapping_shr(32u32.wrapping_sub(n));
        r.win = r.win.wrapping_shl(n);
        let i = c.wrapping_sub(base.get(n as usize).copied().unwrap_or(0) as u32) as usize;
        let sym = leaves.get(i).copied().unwrap_or(0);
        if r.bits < 0 {
            r.refill();
            r.win = r.acc.wrapping_shl((-r.bits) as u32);
            r.bits += 16;
        }
        if sym as u32 != clue {
            out.push(sym);
            continue;
        }
        let run = r.number().wrapping_sub(4);
        if run != 0 {
            // the byte before (before the buffer when nothing is written yet: 0 here)
            let b = if out.len() > start { out[out.len() - 1] } else { 0 };
            out.extend(std::iter::repeat_n(b, run as usize));
            continue;
        }
        let w = r.win;
        r.take(w, 1);
        if (w as i32) >= 0 {
            let w = r.win;
            r.take(w, 8);
            out.push((w >> 24) as u8);
            continue;
        }
        break;
    }
    let end = start + size as usize;
    if out.len() < end && (ty == 0x32fb || ty == 0x34fb) {
        out.resize(end, 0);
    }
    if ty == 0x32fb {
        let mut sum = 0u8;
        for b in &mut out[start..end] {
            sum = sum.wrapping_add(*b);
            *b = sum;
        }
    } else if ty == 0x34fb {
        let (mut d, mut sum) = (0u8, 0u8);
        for b in &mut out[start..end] {
            d = d.wrapping_add(*b);
            sum = sum.wrapping_add(d);
            *b = sum;
        }
    }
    size
}

/// `BTree_Decode` 0x2c3540 (types 0x46, 0xc6; 0x47fb skips a 3-byte field): a byte-pair codec. A 3-byte size
/// (at +2 whatever bit 7 says, unlike `Compress_UnpackedSize`), the escape byte, a count, then (byte, left,
/// right) for each byte that stands for a pair. Then bytes: a plain byte is itself; a pair byte is its left and
/// right, each expanded the same way (`BTree_EmitSymbol` 0x2c34d8; the escape byte also counts as a pair there,
/// with whatever its pair entries hold: 0 here); the escape byte is followed by a literal, or 0 for the end.
/// Appends to `out`; returns the size.
pub fn btree_decode(src: &[u8], out: &mut Vec<u8>) -> u32 {
    let mut q = if be(src, 0, 2) == 0x47fb { 5 } else { 2 };
    let size = be(src, q, 3);
    let byte = |q: usize| *src.get(q).unwrap_or(&0);
    let mut mark = [0u8; 256];
    let (mut left, mut right) = ([0u8; 256], [0u8; 256]);
    mark[byte(q + 3) as usize] = 1;
    let count = byte(q + 4);
    q += 5;
    for _ in 0..count {
        let b = byte(q) as usize;
        left[b] = byte(q + 1);
        right[b] = byte(q + 2);
        mark[b] = 0xff;
        q += 3;
    }
    let mut stack = vec![];
    loop {
        if q >= src.len() {
            // past the end (a stream without its end): the game would read on
            return size;
        }
        let b = byte(q);
        q += 1;
        if mark[b as usize] == 0 {
            out.push(b);
        } else if (mark[b as usize] as i8) >= 0 {
            let c = byte(q);
            q += 1;
            if c == 0 {
                return size;
            }
            out.push(c);
        } else {
            stack.push(right[b as usize]);
            stack.push(left[b as usize]);
            while let Some(s) = stack.pop() {
                if mark[s as usize] == 0 {
                    out.push(s);
                } else {
                    stack.push(right[s as usize]);
                    stack.push(left[s as usize]);
                }
            }
        }
    }
}

/// `Compress_Decode18` 0x2c2d18 (types 0x18, 0x1a, 0x1c and with bit 7): 16-bit pictures in 4×4 blocks. A
/// 3-byte size at +2 (whatever bit 7 says), the width in pixels at +6 (2 bytes), the blocks from +8.
/// `Compress_IsType18` 0x2c2bb8 gives 100 for 0x1afb (else 0) and `Compress_Decode18Rows` 0x2c2bf8 takes the
/// blocks 16 bytes apart (skipping 8) only for 3 or 5, so every type reads them 8 bytes apart. Writes rows of
/// blocks until the size is reached (a width not a multiple of 4 leaves the rows' ends as they were: zeros
/// here). Returns the size.
pub fn decode18(src: &[u8], out: &mut Vec<u8>) -> u32 {
    let size = be(src, 2, 3);
    let width = be(src, 6, 2);
    let mode = if be(src, 0, 2) == 0x1afb { 100 } else { 0 };
    decode18_rows(&src[8.min(src.len())..], out, width as i32, size as i32, mode);
    size
}

/// `Compress_Decode18Rows` 0x2c2bf8.
pub fn decode18_rows(src: &[u8], out: &mut Vec<u8>, width: i32, size: i32, mode: i32) {
    let start = out.len();
    let (step, skip) = if mode == 3 || mode == 5 { (16, 8) } else { (8, 0) };
    let (mut s, mut o) = (skip, 0usize);
    let mut done = 0i32;
    if size <= 0 {
        return;
    }
    loop {
        let mut x = 0;
        if width > 0 {
            loop {
                x += 4;
                let mut block = [0u8; 8];
                for (k, b) in block.iter_mut().enumerate() {
                    *b = *src.get(s + k).unwrap_or(&0);
                }
                let need = start + o + 3 * 2 * width as usize + 8;
                if out.len() < need {
                    out.resize(need, 0);
                }
                decode18_block(&mut out[start + o..], 2 * width as usize, &block);
                o += 8;
                s += step;
                if x >= width {
                    break;
                }
            }
        }
        done += width * 8;
        o = (o as i32 + width * 6) as usize;
        if done >= size {
            break;
        }
    }
}

/// 0x2c27c8: one 4×4 block (two RGB565 colours, then a byte of 2-bit picks per row) to 16-bit PS2 pixels
/// (`0x8000 | b << 10 | g << 5 | r`), `stride` bytes between rows. With colour 0 above colour 1 the other two are
/// (5·c0 + 3·c1) / 8 and (3·c0 + 5·c1) / 8 per channel; else their average and 0 (clear). A colour 0 or 1 of 0
/// becomes 1 first (so never clear); other 0s stay 0.
fn decode18_block(out: &mut [u8], stride: usize, block: &[u8; 8]) {
    let c0 = u16::from_le_bytes([block[0], block[1]]) as i32;
    let c1 = u16::from_le_bytes([block[2], block[3]]) as i32;
    let mix = |a: i32, b: i32, m: i32| (((a & m) * 5 + (b & m) * 3) >> 3) & m;
    let (c2, c3) = if c1 < c0 {
        (mix(c0, c1, 0xf800) | mix(c0, c1, 0x7e0) | mix(c0, c1, 0x1f), mix(c1, c0, 0xf800) | mix(c1, c0, 0x7e0) | mix(c1, c0, 0x1f))
    } else {
        
            ((((c0 & 0xf800) + (c1 & 0xf800)) >> 1) & 0xf800 | (((c0 & 0x7e0) + (c1 & 0x7e0)) >> 1) & 0x7e0 | ((c0 & 0x1f) + (c1 & 0x1f)) >> 1, 0)
    };
    let ps2 = |c: i32| -> u16 {
        let c = c as u16;
        if c == 0 {
            0
        } else {
            (c & 0x1f) << 10 | c >> 1 & 0x3e0 | 0x8000 | c >> 11
        }
    };
    let pal = [ps2(if c0 == 0 { 1 } else { c0 }), ps2(if c1 == 0 { 1 } else { c1 }), ps2(c2), ps2(c3)];
    for row in 0..4 {
        let b = block[4 + row];
        for k in 0..4 {
            let v = pal[(b >> (2 * k) & 3) as usize].to_le_bytes();
            out[row * stride + 2 * k] = v[0];
            out[row * stride + 2 * k + 1] = v[1];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_stream() {
        // 0x10fb, size 8: 4 literals "abcd" (0xe0), copy 4 from 4 back (0x04 0x03: LLL=1 -> 4, offset 3+1), end
        let src = [0x10, 0xfb, 0, 0, 8, 0xe0, b'a', b'b', b'c', b'd', 0x04, 0x03, 0xfc];
        assert_eq!(unpacked_size(&src), 8);
        assert_eq!(decode(&src).unwrap(), b"abcdabcd");
        assert_eq!(load(b"plain".to_vec()).unwrap(), b"plain");
    }

    /// Needs the player's own image: every packed entry of the character archives unpacks to its size.
    #[test]
    fn character_archives_unpack() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let mut files = crate::big::Files::new(crate::disc::Disc::open(&iso).unwrap());
        files.mount_game_archives().unwrap();
        let list: Vec<String> = files.bigs.iter().filter(|b| b.path.starts_with("data/char/"))
            .flat_map(|b| crate::big::entries(&b.header).into_iter().map(move |e| format!("{}|{}", b.path, e.name))).collect();
        assert!(!list.is_empty());
        for p in list {
            let f = files.open(&p, false).unwrap();
            let raw = files.read(&f).unwrap();
            let size = unpacked_size(&raw) as usize;
            let data = load(raw).unwrap();
            assert!(size == 0 || data.len() == size, "{p}");
        }
    }
}

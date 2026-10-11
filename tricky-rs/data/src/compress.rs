//! EA's compressed files (board row E2b): `Compress_UnpackedSize` 0x2c2fb0, `Compress_Decode` 0x2c2de0 and
//! `RefPack_Decode` 0x2c31b0, as the game's file loader (0x2c8938) uses them.
//!
//! A packed file starts with a type byte and 0xfb. `Compress_Decode` hands the type (low bit cleared) to one
//! of EA's codecs: RefPack (0x10, 0x90), Huffman (0x30, 0x32, 0x34, 0xb0, 0xb2, 0xb4), BTree (0x46, 0xc6),
//! another codec at 0x2c2d18 (0x18, 0x1a, 0x1c, 0x98, 0x9a, 0x9c) and a registered one (0x1e, 0x9e). Every
//! packed file on the disc is RefPack (0x10/0x11), so only RefPack is ported here; the others are row E2c.

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
/// does: the unpacked bytes, or `None` when the type is not one the game decodes (or not ported: see the
/// module notes).
pub fn decode(src: &[u8]) -> Option<Vec<u8>> {
    if src.len() < 2 || src[1] != 0xfb {
        return None;
    }
    match src[0] & 0xfe {
        0x10 | 0x90 => {
            let mut out = Vec::with_capacity(unpacked_size(src) as usize);
            refpack_decode(src, &mut out);
            Some(out)
        }
        _ => None,
    }
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

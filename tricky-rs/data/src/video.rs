//! The small films (board row G11a1): `.SSS` files (`DATA/VIDEO/*.SSS`, the courses' previews; the trick book's in
//! `DATA/VIDEO/TB/*.BIG`), as `cVideoStreamMan` plays them.
//!
//! A film is a run of chunks, each an 8-byte head (the tag `SSSF`, then the chunk's size with the head, little
//! endian) and one frame: a shape (`.ssh` block) of type 0x60, a DXT1 picture. `VideoStreamMan_UpdateFrame`
//! 0x1ffa88 takes the stream's next chunk (EA's stream library, 0x2c7a28) at the film's rate (60 / the frames a
//! second given to `VideoStream_Open` 0x1ff990, 1..30) and hands chunk + 8 to `VideoStreamMan_DecodeFrame`
//! 0x1ff928, which sets the shape's type byte to 0x60, unpacks it with `Shape_Unpack` 0x2c2318 and copies the
//! pixels (past the 16-byte header) into the film's texture.
//!
//! `Shape_Unpack`'s converted types: 0x60 (DXT1), 0x61 and 0x62 (the same 8-byte colour blocks, 16 bytes apart:
//! DXT3 and DXT5 with their alpha halves skipped) become a 16-byte header (the source's, its type byte
//! `Shape_PixelFormatIndex(0xf)`, 3) and 16-bit PS2 pixels (`0x8000 | b << 10 | g << 5 | r`) decoded by
//! `Compress_Decode18Rows` 0x2c2bf8 ([`crate::compress::decode18_rows`]); the buffer is the width × (2 bytes for
//! 0x60, 4 for 0x61 and 0x62) × the height. For 0x61 and 0x62 only its first half is written (2-byte pixels); the
//! game leaves the rest as the allocator gave it, the port zeros (STANDIN; the disc's films are all 0x60). Its
//! packed types (bit 0x80) use the compression codecs and are not read here.

/// A film's chunks: (the tag, the frame's bytes past the 8-byte head), until a chunk doesn't fit
pub fn chunks(film: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= film.len() {
        let tag = [film[pos], film[pos + 1], film[pos + 2], film[pos + 3]];
        let size = u32::from_le_bytes([film[pos + 4], film[pos + 5], film[pos + 6], film[pos + 7]]) as usize;
        if size < 8 || pos + size > film.len() {
            break;
        }
        out.push((tag, &film[pos + 8..pos + size]));
        pos += size;
    }
    out
}

/// A film's frames (its `SSSF` chunks' shapes)
pub fn frames(film: &[u8]) -> Vec<&[u8]> {
    chunks(film).into_iter().filter(|(t, _)| t == b"SSSF").map(|(_, f)| f).collect()
}

/// `Shape_Unpack` 0x2c2318 for an unpacked shape: the converted types 0x60..0x62 decoded (see the module); any other
/// type as it is
pub fn unpack(shape: &[u8]) -> Vec<u8> {
    if shape.len() < 0x10 {
        return shape.to_vec();
    }
    let ty = shape[0];
    let (mode, bytes) = match ty {
        0x60 => (1, 0x10),
        0x61 => (3, 0x20),
        0x62 => (5, 0x20),
        _ => return shape.to_vec(),
    };
    let width = i16::from_le_bytes([shape[4], shape[5]]) as i32;
    let height = i16::from_le_bytes([shape[6], shape[7]]) as i32;
    let size = (width.wrapping_mul(bytes) >> 3).wrapping_mul(height);
    let mut out = shape[..0x10].to_vec();
    let mut pixels = Vec::new();
    crate::compress::decode18_rows(&shape[0x10..], &mut pixels, width, size, mode);
    pixels.resize(size.max(0) as usize, 0);
    out.extend_from_slice(&pixels);
    out[0] = 3;
    out
}

/// `VideoStreamMan_DecodeFrame` 0x1ff928: the frame's pixels (type byte 0x60 first, then unpacked, past the header)
pub fn decode_frame(frame: &[u8]) -> Vec<u8> {
    let mut f = frame.to_vec();
    if let Some(b) = f.first_mut() {
        *b = 0x60;
    }
    let u = unpack(&f);
    u.get(0x10..).map(|p| p.to_vec()).unwrap_or_default()
}

/// A frame's size in pixels (the shape's +4 and +6)
pub fn frame_size(frame: &[u8]) -> (usize, usize) {
    if frame.len() < 8 {
        return (0, 0);
    }
    (i16::from_le_bytes([frame[4], frame[5]]).max(0) as usize, i16::from_le_bytes([frame[6], frame[7]]).max(0) as usize)
}

/// 16-bit PS2 pixels (`a << 15 | b << 10 | g << 5 | r`) as RGBA, 8 bits each (5-bit channels widened, alpha 0 or 255)
pub fn rgba(pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 2);
    for p in pixels.chunks_exact(2) {
        let v = u16::from_le_bytes([p[0], p[1]]);
        let wide = |c: u16| ((c << 3) | (c >> 2)) as u8;
        out.extend_from_slice(&[wide(v & 0x1f), wide(v >> 5 & 0x1f), wide(v >> 10 & 0x1f), if v & 0x8000 != 0 { 255 } else { 0 }]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_and_frames() {
        // two SSSF chunks of a 4 × 4 DXT1 frame: header 16 bytes, one block
        let mut frame = vec![0x60u8, 0x20, 0, 0, 4, 0, 4, 0];
        frame.extend_from_slice(&[0; 8]);
        frame.extend_from_slice(&[0x1f, 0, 0xe0, 0x07, 0, 0x55, 0xaa, 0xff]);
        let mut film = Vec::new();
        for _ in 0..2 {
            film.extend_from_slice(b"SSSF");
            film.extend_from_slice(&((frame.len() + 8) as u32).to_le_bytes());
            film.extend_from_slice(&frame);
        }
        let f = frames(&film);
        assert_eq!(f.len(), 2);
        assert_eq!(frame_size(f[0]), (4, 4));
        let px = decode_frame(f[0]);
        assert_eq!(px.len(), 4 * 4 * 2);
        // row 0 all colour 0: RGB565 0x001f is blue, which the PS2 pixel keeps at bits 10..14
        let first = u16::from_le_bytes([px[0], px[1]]);
        assert_eq!(first, 0x8000 | 0x1f << 10);
        assert_eq!(rgba(&px[..2]), vec![0, 0, 255, 255]);
    }

    #[test]
    fn other_chunks_and_a_bad_size() {
        let mut film = Vec::new();
        film.extend_from_slice(b"XXXX");
        film.extend_from_slice(&12u32.to_le_bytes());
        film.extend_from_slice(&[1, 2, 3, 4]);
        film.extend_from_slice(b"SSSF");
        film.extend_from_slice(&12u32.to_le_bytes());
        film.extend_from_slice(&[5, 6, 7, 8]);
        // a chunk claiming a size under its head ends the walk
        film.extend_from_slice(b"SSSF");
        film.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(chunks(&film).len(), 2);
        assert_eq!(frames(&film), vec![&[5u8, 6, 7, 8][..]]);
    }

    #[test]
    fn other_types_stay() {
        let s = vec![2u8; 0x20];
        assert_eq!(unpack(&s), s);
    }
}

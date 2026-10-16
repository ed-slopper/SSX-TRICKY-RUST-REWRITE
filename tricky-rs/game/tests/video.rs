//! `tricky_data::video` (board row G11a1) against the original in the function runner (TRICKY_ELF; skipped without
//! it): `Shape_Unpack` 0x2c2318 on random shapes of the converted types 0x60..0x62 (sizes in 4s, random blocks) and of
//! other types, and `VideoStreamMan_DecodeFrame` 0x1ff928 into a film's texture; the allocator and free stubbed, the
//! rest (the copy, `Compress_Decode18Rows`, `Shape_PixelFormatIndex`) run as the original. And with TRICKY_DISC every
//! film on the disc decoded.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_data::video::{decode_frame, frames, unpack};

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

const ARENA: u32 = 0x0100_0000;
const SRC: u32 = ARENA;
const OUT: u32 = ARENA + 0x10_0000;
const MGR: u32 = ARENA + 0x20_0000;
const TEX: u32 = ARENA + 0x21_0000;

#[test]
fn shape_unpack_and_decode_frame_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let freed = Rc::new(RefCell::new(Vec::new()));
    r.stub(0x2ccf70, |c, _| c.set_gpr(2, OUT as u64));
    let f2 = freed.clone();
    r.stub(0x2ccfc0, move |c, _| f2.borrow_mut().push(c.gpr(4) as u32));
    for f in [0x2ce114u32, 0x2c2bf8, 0x2c27c8, 0x2c26d8, 0x2c2318] {
        r.allow(f);
    }
    let mut g = Rng(0x2c23_0000_0000_0001);
    for case in 0..3000u32 {
        let ty = [0x60u8, 0x60, 0x61, 0x62, 0x02, 0x05][g.below(6) as usize];
        let (w, h) = (4 * (1 + g.below(16)), 4 * (1 + g.below(16)));
        let blocks = (w / 4) * (h / 4) * if ty == 0x60 { 1 } else { 2 };
        let mut shape = vec![ty, g.next() as u8, g.next() as u8, g.next() as u8];
        shape.extend_from_slice(&(w as i16).to_le_bytes());
        shape.extend_from_slice(&(h as i16).to_le_bytes());
        for _ in 0..8 {
            shape.push(g.next() as u8);
        }
        for _ in 0..blocks * 8 {
            shape.push(g.next() as u8);
        }
        let direct = g.below(2) == 0;
        r.mem.write(SRC, &shape).unwrap();
        // the output buffer and the texture start as a pattern
        let out_len = (w * 4 * h + 0x40) as usize;
        r.mem.write(OUT, &vec![0xa5u8; out_len]).unwrap();
        r.mem.write(TEX, &vec![0x5au8; out_len]).unwrap();
        freed.borrow_mut().clear();
        if direct {
            r.cpu.set_gpr(4, SRC as u64);
            r.call(0x2c2318).unwrap_or_else(|e| panic!("case {case}: {e}"));
            let ret = r.cpu.gpr(2) as u32;
            let mine = unpack(&shape);
            if (0x60..=0x62).contains(&ty) {
                assert_eq!(ret, OUT, "case {case}: the buffer");
                // (0x61 and 0x62 fill a buffer of 4 bytes a pixel with 2-byte pixels: the rest is what the allocator
                // gave, zeros in the port)
                let written = 0x10 + (w * 2 * h) as usize;
                let mut theirs = vec![0u8; written];
                r.mem.read(OUT, &mut theirs).unwrap();
                assert_eq!(theirs, mine[..written], "case {case}: type {ty:#x} {w}x{h}");
                assert_eq!(mine.len(), 0x10 + (w * if ty == 0x60 { 2 } else { 4 } * h) as usize);
            } else {
                assert_eq!(ret, SRC, "case {case}: an unconverted shape is used as it is");
                assert_eq!(mine, shape);
            }
        } else {
            // the film's manager: +0x94 its texture (a shape: 16-byte header, then the pixels), +0x98 the pixels'
            // size; DecodeFrame sets the type byte itself
            let size = w * 2 * h;
            r.mem.write_u32(MGR + 0x94, TEX);
            r.mem.write_u32(MGR + 0x98, size);
            r.cpu.set_gpr(4, MGR as u64);
            r.cpu.set_gpr(5, SRC as u64);
            r.call(0x1ff928).unwrap_or_else(|e| panic!("case {case}: {e}"));
            let mut tex = vec![0u8; size as usize];
            r.mem.read(TEX + 0x10, &mut tex).unwrap();
            let mut frame = shape.clone();
            frame[0] = 0x60;
            let mine = decode_frame(&frame);
            assert_eq!(&tex[..], &mine[..size as usize], "case {case}: {w}x{h}");
            assert_eq!(r.mem.read_u32(MGR + 0x90), 1, "case {case}: the frame's flag");
            assert_eq!(freed.borrow().as_slice(), &[OUT], "case {case}: the unpacked shape freed");
        }
    }
}

#[test]
fn every_film_on_the_disc_decodes() {
    let Ok(path) = std::env::var("TRICKY_DISC") else { return };
    let mut disc = tricky_data::disc::Disc::open(&path).expect("the disc");
    let mut films = 0;
    let mut decoded = 0usize;
    let mut check = |name: &str, film: &[u8]| {
        let fr = frames(film);
        assert!(!fr.is_empty(), "{name}: no frames");
        for f in fr {
            let (w, h) = tricky_data::video::frame_size(f);
            assert!(w > 0 && h > 0 && w % 4 == 0 && h % 4 == 0, "{name}: {w}x{h}");
            assert_eq!(decode_frame(f).len(), w * h * 2, "{name}");
            decoded += 1;
        }
        films += 1;
    };
    for name in ["GARI", "SNOW", "ELYSIUM", "MESA", "MERQUER", "ALOHA", "PIPE", "MEGAPLE", "ALASKA", "JSNOW", "JPIPE", "JMEGAPLE", "JELYSIUM", "JMESA", "JALASKA", "JMERQUER"] {
        if let Some(film) = disc.read_path(&format!("/DATA/VIDEO/{name}.SSS")).ok().flatten() {
            check(name, &film);
        }
    }
    for name in ["ELIS", "MOBY", "BROD", "KAOR", "SEEI", "ZOE", "MARI", "MAC", "EDDI", "LUTH", "PSYM", "JP"] {
        let Some(big) = disc.read_path(&format!("/DATA/VIDEO/TB/TB_{name}.BIG")).ok().flatten() else { continue };
        let header = &big[..tricky_data::big::header_size(&big).min(big.len())];
        for e in tricky_data::big::entries(header) {
            let film = &big[e.offset as usize..(e.offset + e.size) as usize];
            check(&e.name, film);
        }
    }
    println!("{films} films, {decoded} frames");
    assert!(films > 20, "only {films} films found");
}

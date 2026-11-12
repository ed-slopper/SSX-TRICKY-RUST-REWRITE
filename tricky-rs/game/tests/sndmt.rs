//! The MicroTalk decoder (`tricky_game::sndmt`) on the announcer's lines from the disc (TRICKY_DISC), and against
//! the original's `SndMt_ParseHeader` / `SndMt_DecodeFrame` in the function runner (TRICKY_ELF too).

use tricky_game::sndmt::{self, Bits, Mt, FRAME};
use tricky_game::speechhost::SpeechHost;

type Files = tricky_data::big::Files;

fn disc() -> Option<Files> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let mut files = Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount_game_archives().ok()?;
    Some(files)
}

/// Some lines the announcer plays: (file bytes, part offset).
fn lines() -> Option<Vec<(Vec<u8>, usize)>> {
    let mut files = disc()?;
    let inf = files.load("data/config/speech.inf").ok().flatten()?;
    let mut load = disc()?;
    let mut h = SpeechHost::new(&inf, Box::new(move |p: &str| load.load(p).ok().flatten()));
    let mut out = Vec::new();
    for step in 0..72u32 {
        let mut ev = [0u32; 20];
        let new = (step / 4 % 6) as i64;
        ev[0] = 0x1002049;
        ev[1] = step / 24;
        ev[2] = tricky_game::speech::place_mask(new as i32 + 1, new);
        ev[3] = step % 4;
        h.post(&ev);
        h.update();
        for p in h.take_plays() {
            let bytes = files.load(&p.file).ok().flatten().unwrap_or_else(|| panic!("{}", p.file));
            let s = sndmt::decode(&bytes, p.offset as usize).expect("a MicroTalk part");
            h.line_started(p.handle, (s.samples.len() / s.channels) as f32 * 1000.0 / s.rate as f32);
            out.push((bytes, p.offset as usize));
        }
        h.frame(10.0);
    }
    Some(out)
}

#[test]
fn announcer_lines_decode() {
    let Some(lines) = lines() else { return };
    assert!(!lines.is_empty());
    for (bytes, at) in &lines {
        let s = sndmt::decode(bytes, *at).expect("a MicroTalk part");
        assert!(s.rate > 8000 && s.channels >= 1);
        let peak = s.samples.iter().map(|x| (*x as i32).abs()).max().unwrap();
        let clipped = s.samples.iter().filter(|x| **x == i16::MIN || **x == i16::MAX).count();
        assert!(s.samples.len() > 4000 && peak > 1000 && clipped * 100 < s.samples.len(), "{} samples, peak {peak}", s.samples.len());
    }
}

mod against_the_original {
    use super::*;
    use r5900::Runner;

    const PARSE_HEADER: u32 = 0x2e3070;
    const DECODE_FRAME: u32 = 0x2e31e8;
    const BITS_READ: u32 = 0x2e2b98;
    const BITS_SKIP: u32 = 0x2e2c00;
    const READ_EXCITATION: u32 = 0x2e2c50;
    const INTERPOLATE: u32 = 0x2e2e98;
    const RC_TO_LPC: u32 = 0x2e2f10;
    const SYNTH: u32 = 0x2e3010;
    const LPC_SYNTH: u32 = 0x2e3620;
    const MEMCOPY: u32 = 0x2decac;
    const DATA: u32 = 0x1100000;
    const ST: u32 = 0x1000000;

    fn state(r: &mut Runner) -> Vec<u8> {
        let mut b = vec![0u8; 0xd40];
        r.mem.read(ST, &mut b).unwrap();
        b
    }

    fn ours(mt: &Mt) -> Vec<u8> {
        let mut b = Vec::with_capacity(0xd40);
        let w = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
        w(&mut b, DATA + mt.bits.pos as u32);
        w(&mut b, mt.bits.buf);
        w(&mut b, mt.bits.count);
        w(&mut b, mt.threshold as u32);
        for f in mt.gains.iter().chain(&mt.rc).chain(&mt.hist).chain(&mt.a) {
            w(&mut b, f.to_bits());
        }
        b
    }

    /// Every block of the first channel of each line, header and frames, decoded by both; the whole state
    /// compared after each frame.
    #[test]
    fn frames_decode_as_the_original() {
        let Some(lines) = lines() else { return };
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        for f in [PARSE_HEADER, DECODE_FRAME, BITS_READ, BITS_SKIP, READ_EXCITATION, INTERPOLATE, RC_TO_LPC, SYNTH, LPC_SYNTH, MEMCOPY] {
            r.allow(f);
        }
        let mut frames = 0;
        for (n, (bytes, at)) in lines.iter().enumerate() {
            let word = |p: usize| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap());
            let mut mt = Box::new(Mt::default());
            r.mem.write(ST, &vec![0u8; 0xd40]).unwrap();
            let h = sndmt::header(bytes, *at).unwrap();
            let (nch, mut p) = (h.channels, h.end);
            let mut left = 0usize;
            while &bytes[p..p + 4] != b"SCEl" {
                let size = word(p + 4) as usize;
                if &bytes[p..p + 4] == b"SCDl" {
                    let mut count = (word(p + 8) & 0x7fff_ffff) as usize;
                    let start = p + 12 + 4 * nch + word(p + 12) as usize; // the first channel
                    let block = &bytes[start..p + size];
                    r.mem.write(DATA, block).unwrap();
                    r.mem.write(DATA + block.len() as u32, &[0u8; 16]).unwrap();
                    if block[0] == 0 {
                        mt.bits = Bits::at(block, 1);
                        r.mem.write_u32(ST, DATA + 2);
                        r.mem.write_u32(ST + 4, block[1] as u32);
                        r.mem.write_u32(ST + 8, 8);
                    } else {
                        mt.parse_header(block, 1);
                        r.cpu.set_gpr(4, (DATA + 1) as u64);
                        r.cpu.set_gpr(5, ST as u64);
                        r.call(PARSE_HEADER).unwrap();
                        assert_eq!(state(&mut r), ours(&mt), "line {n}: header");
                        left = 0;
                    }
                    while count > 0 {
                        if left == 0 {
                            mt.decode_frame(block);
                            r.cpu.set_gpr(4, ST as u64);
                            r.call(DECODE_FRAME).unwrap();
                            let (a, b) = (state(&mut r), ours(&mt));
                            if a != b {
                                let at = (0..a.len()).step_by(4).find(|&i| a[i..i + 4] != b[i..i + 4]).unwrap();
                                panic!("line {n}, frame {frames}: first differs at +{at:#x}: original {:x?}, ours {:x?}", &a[at..at + 4], &b[at..at + 4]);
                            }
                            frames += 1;
                            left = FRAME;
                        }
                        let take = count.min(left);
                        left -= take;
                        count -= take;
                    }
                }
                p += size;
            }
        }
        assert!(frames > 20, "{frames} frames");
    }
}

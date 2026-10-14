//! The two small loaders left of the audio config (board row G10j2): `Audio_LoadChantBanks` 0x22dce0, which reads the
//! chant sections of an ini file into the audio object (twenty rows of thirteen 0x40-byte name strings from +4, and two
//! ints at +0x4104 / +0x4108), and `SoundMenu_BuildMusicList` 0x187f58, which lists the enabled songs for the sound
//! menu. The callees are [`LoadEnv`] calls by address.

use crate::speechlib::Mem;
use crate::speechload::*;

pub const FIND_SECTION2: u32 = 0x221af0;
pub const READ_INT: u32 = 0x221fd8;
pub const COUNT_ENABLED: u32 = 0x225588;
pub const NEXT_ENABLED: u32 = 0x225660;
pub const SELECT_SONG: u32 = 0x2255c0;

const S_INTS: u32 = 0x3a18d8;
const S_ROWS: u32 = 0x3a1900;
const I_A: u32 = 0x3a18e0;
const I_B: u32 = 0x3a18f0;
const F_FIRST: u32 = 0x3a1908;
const F_FIRST_OUT: u32 = 0x3a1910;
/// The key format, and where its string goes in a row (`obj + row * 0x40 + offset`); the thirteenth is the fall-through.
pub const KEYS: [(u32, u32); 12] = [
    (0x3a1918, 0x1404),
    (0x3a1920, 0x2304),
    (0x3a1928, 0x504),
    (0x3a1930, 0x1e04),
    (0x3a1938, 0x1904),
    (0x3a1940, 0xa04),
    (0x3a1950, 0x4),
    (0x3a1958, 0x3204),
    (0x3a1960, 0x2804),
    (0x3a1970, 0x3704),
    (0x3a1980, 0x2d04),
    (0x3a1990, 0x3c04),
];

/// 0x22dce0, with `sp` the frame after the prologue (the ini reader at +0, the key at +0x810, the string at +0x820).
pub fn load_chant_banks(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, obj: u32, file: u32) {
    let (key, val) = (sp + 0x810, sp + 0x820);
    e.call(m, INI_OPEN, &[sp, file, 0x10]);
    m.w32(obj + 0x4108, 0xffff_ffff);
    m.w32(obj + 0x4104, 0xffff_ffff);
    let mut found = e.call(m, FIND_SECTION2, &[sp, S_INTS]) != 0;
    if found {
        while e.call(m, AT_END, &[sp]) == 0 {
            e.call(m, READ_TOKEN, &[sp]);
            e.call(m, READ_INT, &[sp, I_A, obj + 0x4104]);
            e.call(m, READ_INT, &[sp, I_B, obj + 0x4108]);
        }
    }
    found = e.call(m, FIND_SECTION2, &[sp, S_ROWS]) != 0;
    if found {
        while e.call(m, AT_END, &[sp]) == 0 {
            let tok = e.call(m, READ_TOKEN, &[sp]);
            let mut row = 0u32;
            while (row as i32) < 0x14 {
                let n = row + 1;
                e.call(m, SPRINTF, &[key, F_FIRST, n]);
                if e.call(m, STRCMP, &[tok, key]) == 0 {
                    e.call(m, READ_STRING, &[sp, tok, val]);
                    e.call(m, SPRINTF, &[obj + row * 0x40 + 0xf04, F_FIRST_OUT, val]);
                    break;
                }
                let mut hit = None;
                for (fmt, off) in KEYS.iter() {
                    e.call(m, SPRINTF, &[key, *fmt, n]);
                    if e.call(m, STRCMP, &[tok, key]) == 0 {
                        hit = Some(*off);
                        break;
                    }
                }
                if let Some(off) = hit {
                    e.call(m, READ_STRING, &[sp, tok, val]);
                    e.call(m, SPRINTF, &[obj + row * 0x40 + off, val]);
                    break;
                }
                row = n;
            }
        }
    }
    e.call(m, INI_CLOSE, &[sp, 2]);
}

/// 0x187f58: `o` is the sound menu object.
pub fn build_music_list(m: &mut dyn Mem, e: &mut dyn LoadEnv, o: u32) {
    let sys = m.r32(0x343050);
    let count = e.call(m, COUNT_ENABLED, &[sys]) as i32;
    if count < 1 {
        m.w32(o + 0x140, 0);
    } else {
        let mut i = 0i32;
        loop {
            let sys = m.r32(0x343050);
            let cur = m.r32(sys.wrapping_add(0x36e8));
            m.w32(o.wrapping_add(0x148).wrapping_add((i as u32).wrapping_mul(8)), i as u32);
            m.w32(
                o.wrapping_add(0x144).wrapping_add((i as u32).wrapping_mul(8)),
                sys.wrapping_add(cur.wrapping_shl(5)).wrapping_add(0x12e0),
            );
            i += 1;
            e.call(m, NEXT_ENABLED, &[sys]);
            if i >= count {
                break;
            }
        }
        m.w32(o + 0x140, 0);
    }
    let sys = m.r32(0x343050);
    e.call(m, SELECT_SONG, &[sys, 0]);
    m.w32(o + 0x408, count as u32);
    m.w32(o + 0x40c, o + 0x144);
    m.w32(o + 0x410, o + 0x140);
    m.w32(o + 0x404, 0x38_9030);
}

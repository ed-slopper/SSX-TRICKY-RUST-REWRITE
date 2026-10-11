//! Audio settings from the config files as the game reads them (board row E5b2), on [`crate::ini::IniReader`]:
//! the mix levels of `DATA/CONFIG/AUDIO.INF` (`Audio_LoadVolumeLevels` 0x21ca20) and the course's intro music of
//! `DATA/CONFIG/INTROMUS.INF` (`Music_StartCourseSong` 0x214e60). Checked against the game's own functions in the
//! function runner (`tricky-game/tests/ini.rs`).

use crate::ini::IniReader;

/// The mix levels (0x343054..0x34306c): SFX, music, rider speech, announcer, jukebox, uber, and how far the intro
/// music ducks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub sfx: f32,
    pub music: f32,
    pub char_speech: f32,
    pub pa_speech: f32,
    pub jukebox: f32,
    pub uber: f32,
    pub intro_duck: f32,
}

const KEYS: [&[u8]; 7] = [b"SFXLEVEL", b"MUSICLEVEL", b"CHARSPCHLEVEL", b"PASPCHLEVEL", b"JUKEBOXLEVEL", b"UBERLEVEL", b"INTRODUCK"];

/// `Audio_LoadVolumeLevels` 0x21ca20: the keys of section `scaling` (`IGLEVELSCALING` in a race, `FELEVELSCALING`
/// in the menus), then of the course's section (`course`, upper-cased: searched on from there, so it must come
/// later in the file); each level the last value read / 100 when it is 0..200 (the duck 0..100), else 1.0 (the
/// duck 0.75). A key whose value has no number leaves the level as it was (the game reads its stack word).
pub fn levels(bytes: &[u8], scaling: &[u8], course: &[u8]) -> Levels {
    let mut v = [u32::MAX; 7];
    let mut r = IniReader::new(bytes);
    for section in [scaling, course] {
        if r.find_section(section) {
            while !r.at_section_end() {
                r.read_token();
                for (k, key) in KEYS.iter().enumerate() {
                    if let Some(Some(n)) = r.read_int(key) {
                        v[k] = n as u32;
                    }
                }
            }
        }
    }
    let lvl = |x: u32, max: u32, def: f32| if x < max { x as i32 as f32 / 100.0 } else { def };
    Levels {
        sfx: lvl(v[0], 0xc9, 1.0),
        music: lvl(v[1], 0xc9, 1.0),
        char_speech: lvl(v[2], 0xc9, 1.0),
        pa_speech: lvl(v[3], 0xc9, 1.0),
        jukebox: lvl(v[4], 0xc9, 1.0),
        uber: lvl(v[5], 0xc9, 1.0),
        intro_duck: lvl(v[6], 0x65, 0.75),
    }
}

/// `Music_StartCourseSong` 0x214e60: the intro music's stream for a course (its name, lower-cased by the game):
/// the first section whose name (lower-cased) the course's contains and which has something in it, its first word
/// `SONG = "file"`, as `data/audio/<file>`. `None`: no such section, or its first word is not SONG.
pub fn intro_song(bytes: &[u8], course: &[u8]) -> Option<Vec<u8>> {
    let course: Vec<u8> = course.iter().map(|c| c.to_ascii_lowercase()).collect();
    let mut r = IniReader::new(bytes);
    loop {
        let name = r.next_section()?;
        let name: Vec<u8> = name.iter().map(|c| c.to_ascii_lowercase()).collect();
        let inside = name.is_empty() || course.windows(name.len()).any(|w| w == name.as_slice());
        if inside && !r.at_section_end() {
            break;
        }
    }
    let tok = r.read_token();
    if !tok.eq_ignore_ascii_case(b"SONG") {
        return None;
    }
    let s = r.read_string(b"SONG").unwrap_or_default();
    Some([b"data/audio/".as_slice(), &s].concat())
}

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

/// One bank file for the sound manager (`BankManager_LoadIfChanged` 0x20a460): the slot and the path (the section's
/// base path and the file's name, joined as `%s%s`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BankLoad {
    pub slot: u32,
    pub path: Vec<u8>,
}

/// What `Audio_LoadInGameConfig` takes from BANKS.INF for a course.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct IngameBanks {
    /// The loads in order.
    pub loads: Vec<BankLoad>,
    /// The `SWAP` files, kept in the audio system's list at +0x38e0 (0x40 bytes each, the count at +0x38dc).
    pub swaps: Vec<Vec<u8>>,
}

/// The `[GLOBAL]` section's last `BASEPATH`, then the cursor back to the top (empty without one: the game's buffer
/// is then its stack's).
fn global_base_path(r: &mut IniReader) -> Vec<u8> {
    let mut base = Vec::new();
    if r.find_section(b"GLOBAL") {
        while !r.at_section_end() {
            r.read_token();
            if let Some(s) = r.read_string(b"BASEPATH") {
                base = s;
            }
        }
    }
    r.rewind();
    base
}

/// `Audio_LoadInGameConfig` 0x211098, its BANKS.INF half: every section whose name contains the course's (the course
/// name upper-cased; `strstr`, so case matters) is read key by key: `BASEPATH` changes the section's base path,
/// `MAIN`, `BOARD`, `BANK`, `CROWD`, `AUX` and `TRICKY` load the file into slots 0 to 5, `SWAP` adds it to the swap
/// list. A key whose value does not read keeps the last file name read.
pub fn ingame_banks(bytes: &[u8], course: &[u8]) -> IngameBanks {
    ingame_banks_checked(bytes, course).0
}

/// [`ingame_banks`] and whether a file name was used before any had been read (the game then uses what its stack
/// held, which no result can tell).
pub fn ingame_banks_checked(bytes: &[u8], course: &[u8]) -> (IngameBanks, bool) {
    const KEYS: [(&[u8], u32); 6] = [(b"MAIN", 0), (b"BOARD", 1), (b"BANK", 2), (b"CROWD", 3), (b"AUX", 4), (b"TRICKY", 5)];
    let course = course.to_ascii_uppercase();
    let mut r = IniReader::new(bytes);
    let global = global_base_path(&mut r);
    let mut out = IngameBanks::default();
    let mut file: Vec<u8> = Vec::new();
    let mut have_file = false;
    let mut stale = false;
    while let Some(name) = r.next_section() {
        if !(course.is_empty() || name.windows(course.len()).any(|w| w == course.as_slice())) {
            continue;
        }
        let mut base = global.clone();
        while !r.at_section_end() {
            r.read_token();
            if let Some(s) = r.read_string(b"BASEPATH") {
                base = s;
            }
            // (the game compares the token buffer, which a read `BASEPATH` string has overwritten)
            let tok = r.token().to_vec();
            let join = |base: &[u8], file: &[u8]| [base, file].concat();
            if let Some((key, slot)) = KEYS.iter().find(|(k, _)| tok.eq_ignore_ascii_case(k)) {
                if let Some(s) = r.read_string(key) {
                    file = s;
                    have_file = true;
                }
                stale |= !have_file;
                out.loads.push(BankLoad { slot: *slot, path: join(&base, &file) });
            } else if tok.eq_ignore_ascii_case(b"SWAP") {
                if let Some(s) = r.read_string(b"SWAP") {
                    file = s;
                    have_file = true;
                }
                stale |= !have_file;
                out.swaps.push(join(&base, &file));
            }
        }
    }
    (out, stale)
}

/// `Audio_LoadFEConfig` 0x20f160, its BANKS.INF half: section `FE`, `BASEPATH` as above, `MAIN` into slot 0 and
/// `CHARSPCH<n>` (the front end's speech set `n`) into slot 7.
pub fn fe_banks(bytes: &[u8], speech: i32) -> Vec<BankLoad> {
    fe_banks_checked(bytes, speech).0
}

/// [`fe_banks`] and whether a file name was used before any had been read (see [`ingame_banks_checked`]).
pub fn fe_banks_checked(bytes: &[u8], speech: i32) -> (Vec<BankLoad>, bool) {
    let mut r = IniReader::new(bytes);
    let mut base = global_base_path(&mut r);
    let mut out = Vec::new();
    let mut stale = false;
    if r.find_section(b"FE") {
        let key = format!("CHARSPCH{speech}").into_bytes();
        let mut file: Vec<u8> = Vec::new();
        let mut have_file = false;
        while !r.at_section_end() {
            r.read_token();
            if let Some(s) = r.read_string(b"BASEPATH") {
                base = s;
            }
            let tok = r.token().to_vec();
            if tok.eq_ignore_ascii_case(b"MAIN") {
                if let Some(s) = r.read_string(b"MAIN") {
                    file = s;
                    have_file = true;
                }
                stale |= !have_file;
                out.push(BankLoad { slot: 0, path: [base.as_slice(), &file].concat() });
            } else if tok.eq_ignore_ascii_case(&key) {
                if let Some(s) = r.read_string(&key) {
                    file = s;
                    have_file = true;
                }
                stale |= !have_file;
                out.push(BankLoad { slot: 7, path: [base.as_slice(), &file].concat() });
            }
        }
    }
    (out, stale)
}

//! The music configuration as the game reads it (board row E5b): `MusicSys_Init` 0x224a98 reading
//! `DATA/CONFIG/MUSIC.INF` (the song table) and `MusicSys_LoadMusicMap` 0x224cf8 reading `DATA/CONFIG/MUSICMAP.INF`
//! (which songs a course plays), on [`crate::ini::IniReader`]. Checked against the game's own functions in the
//! function runner (`tricky-game/tests/ini.rs`).

use crate::ini::IniReader;

/// One song of the table (`cMusicSys` +0x12e0 + i·0x20 the name, +0x16e0 + i·0x100 the path).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Song {
    /// the section's name ("Gin & Sin")
    pub name: Vec<u8>,
    /// BASEPATH + LOOPDATA, the loop bank (`None` where the section has no LOOPDATA: the game leaves the slot as it
    /// was)
    pub loop_path: Option<Vec<u8>>,
    /// PATHDATA: the Pathfinder data's file (not read by `MusicSys_Init`; ours, to find the song's audio)
    pub path_data: Option<Vec<u8>>,
}

/// The song table (`cMusicSys`: +0x1260 the base path, +0x36e0 the count, +0x36ec the enabled mask).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MusicTable {
    pub base: Vec<u8>,
    pub songs: Vec<Song>,
    /// every song read is enabled at first
    pub mask: u32,
}

fn eq_ignore_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || hay.windows(needle.len()).any(|w| w == needle)
}

/// `MusicSys_Init` 0x224a98: `[GLOBAL]`'s BASEPATH, then from the top every other section (32 at most) a song, its
/// path BASEPATH + the first LOOPDATA in it.
pub fn music_table(bytes: &[u8]) -> MusicTable {
    let mut t = MusicTable::default();
    let mut r = IniReader::new(bytes);
    if r.find_section(b"GLOBAL") {
        while !r.at_section_end() {
            r.read_token();
            if let Some(s) = r.read_string(b"BASEPATH") {
                t.base = s;
            }
        }
    }
    let mut r = IniReader::new(bytes);
    while t.songs.len() <= 0x1f {
        let Some(name) = r.next_section() else { break };
        if eq_ignore_case(&name, b"GLOBAL") {
            continue;
        }
        t.mask |= 1 << t.songs.len();
        let mut song = Song { name, ..Default::default() };
        // ours alongside: the PATHDATA before the LOOPDATA, read from a copy so the game's walk is unchanged
        let mut look = r.clone();
        while !look.at_section_end() {
            let tok = look.read_token();
            if eq_ignore_case(&tok, b"PATHDATA") {
                song.path_data = look.read_string(b"PATHDATA");
                break;
            }
        }
        while !r.at_section_end() {
            let tok = r.read_token();
            if eq_ignore_case(&tok, b"LOOPDATA") {
                let mut p = t.base.clone();
                if let Some(s) = r.read_string(b"LOOPDATA") {
                    p.extend_from_slice(&s);
                }
                song.loop_path = Some(p);
                break;
            }
        }
        t.songs.push(song);
    }
    t
}

/// `MusicSys_LoadMusicMap` 0x224cf8: the songs a course plays. `course` is the course's name upper-cased (the
/// game's `Course_GetInfo` string; a section matches when either starts with the other); each `SONG = "…"` enables
/// the first song whose name contains it (case matters), four at most; none: the first song. Returns the enabled
/// mask and the count (`cMusicSys` +0x36ec, +0x36e4).
pub fn course_songs(map: &[u8], table: &MusicTable, course: &[u8]) -> (u32, u32) {
    let (mut mask, mut count) = (0u32, 0u32);
    let mut r = IniReader::new(map);
    if r.find_section_matching(course) {
        r.skip_to_next_section();
        let mut last: Vec<u8> = Vec::new();
        while !r.at_section_end() {
            if mask.count_ones() > 3 {
                break;
            }
            let tok = r.read_token();
            if eq_ignore_case(b"SONG", &tok) {
                if let Some(s) = r.read_string(b"SONG") {
                    last = s;
                }
                for (i, song) in table.songs.iter().enumerate() {
                    if contains(&song.name, &last) {
                        mask |= 1 << i;
                        count += 1;
                        break;
                    }
                }
            }
        }
    }
    if mask == 0 {
        mask = 1;
        count += 1;
    }
    (mask, count)
}

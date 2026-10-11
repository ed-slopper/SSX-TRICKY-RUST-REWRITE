//! BIG archives (board row E2a): EA's `C0FB` and `BIGF` archives and the game's way of finding a file in
//! them, ported from its loader (`lib-big`).
//!
//! Both kinds are a big-endian directory of (offset, size, name) followed by the files. The game keeps only
//! an archive's directory in memory: `App_OpenBigFiles` 0x17bdf8 mounts `data/audio/speech.big`,
//! `data/audio/audio.big`, `data/char/mdlps2.big`, `data/char/brdps2.big`, `data/char/anm.big`,
//! `data/char/texps2.big` and `data/audio/music.big` (the audio ones only when the disc has them), and every
//! file open then goes through the rules of the async file system's opener (0x2cb328): see [`Files::open`].
//! Entries may be RefPack-compressed; unpacking them is row E2b.

use crate::disc::{Disc, DiscFile};
use std::io;

/// `BIG_Identify` 0x2cbc88: 1 for `C0FB` (big-endian u16 0xc0fb first), 2 for `BIGF`, 0 for neither.
pub fn identify(b: &[u8]) -> u32 {
    if b.len() >= 2 && u16::from_be_bytes([b[0], b[1]]) == 0xc0fb {
        1
    } else if b.len() >= 4 && &b[..4] == b"BIGF" {
        2
    } else {
        0
    }
}

/// `BIG_HeaderSize` 0x2cbd08: bytes from the start of the archive to the end of its directory. `C0FB`: the
/// u16 at +2, plus 4; `BIGF`: the u32 at +0xc.
pub fn header_size(b: &[u8]) -> usize {
    match identify(b) {
        1 => u16::from_be_bytes([b[2], b[3]]) as usize + 4,
        2 => u32::from_be_bytes([b[0xc], b[0xd], b[0xe], b[0xf]]) as usize,
        _ => 0,
    }
}

/// One directory entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigEntry {
    pub name: String,
    /// From the start of the archive.
    pub offset: u32,
    pub size: u32,
}

/// `BIG_Lookup` 0x2cbdc0: walks the directory (`C0FB` from +6: u24 offset, u24 size, name; `BIGF` from +0x10:
/// u32 offset, u32 size, name; names end with a zero) and returns the entry named `name` (compared with
/// `stricmp`), or with no name the entry number `index`.
pub fn lookup(header: &[u8], name: Option<&str>, index: usize) -> Option<BigEntry> {
    let kind = identify(header);
    let end = header_size(header).min(header.len());
    let (mut p, width) = match kind {
        1 => (6, 3),
        2 => (0x10, 4),
        _ => return None,
    };
    let mut i = 0;
    while p < end {
        let at = p + 2 * width;
        let len = header[at..].iter().position(|&c| c == 0).unwrap_or(header.len() - at);
        let entry_name = &header[at..at + len];
        let hit = match name {
            None => i == index,
            Some(n) => stricmp(entry_name, n.as_bytes()) == 0,
        };
        if hit {
            let be = |at: usize| header[at..at + width].iter().fold(0u32, |v, &b| v << 8 | b as u32);
            return Some(BigEntry { name: String::from_utf8_lossy(entry_name).into_owned(), offset: be(p), size: be(p + width) });
        }
        i += 1;
        p = at + len + 1;
    }
    None
}

/// Every entry, in directory order (`BIG_Lookup` by number until it fails).
pub fn entries(header: &[u8]) -> Vec<BigEntry> {
    (0..).map_while(|i| lookup(header, None, i)).collect()
}

/// `BIG_GetFileData` 0x2cc0a8 for an archive held whole in memory (`cMemoryBigFile`): the entry's bytes. The
/// game takes an offset of 0 as "not found".
pub fn file_data<'a>(big: &'a [u8], name: &str) -> Option<&'a [u8]> {
    let e = lookup(big, Some(name), 0)?;
    if e.offset == 0 {
        return None;
    }
    big.get(e.offset as usize..(e.offset + e.size) as usize)
}

/// `stricmp` 0x2fcc98: letters compared as lower case, the difference at the first mismatch.
fn stricmp(a: &[u8], b: &[u8]) -> i32 {
    let lower = |c: u8| c.to_ascii_lowercase() as i32;
    let mut i = 0;
    loop {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x == 0 || lower(x) != lower(y) {
            return lower(x) - lower(y);
        }
        i += 1;
    }
}

/// A mounted archive: the path it was mounted under, its directory, and where it is on the disc.
pub struct MountedBig {
    pub path: String,
    pub header: Vec<u8>,
    pub file: DiscFile,
}

/// Where an opened file's bytes are: on the disc from `start`, `size` long.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenFile {
    /// Byte offset on the disc.
    pub start: u64,
    pub size: u32,
    /// The archive it came from (its mount path), if any.
    pub big: Option<String>,
}

/// The disc with its mounted archives: what the game's async file system finds a path in.
pub struct Files {
    pub disc: Disc,
    /// Newest first, as the game links them (0x2caca8 puts a new mount at the head of the list).
    pub bigs: Vec<MountedBig>,
}

impl Files {
    pub fn new(disc: Disc) -> Files {
        Files { disc, bigs: Vec::new() }
    }

    /// Mounts the archives `App_OpenBigFiles` 0x17bdf8 mounts, in its order.
    pub fn mount_game_archives(&mut self) -> io::Result<()> {
        for (path, if_there) in [
            ("data/audio/speech.big", true),
            ("data/audio/audio.big", true),
            ("data/char/mdlps2.big", false),
            ("data/char/brdps2.big", false),
            ("data/char/anm.big", false),
            ("data/char/texps2.big", false),
            ("data/audio/music.big", true),
        ] {
            if if_there && self.disc.find(path).is_none() {
                continue;
            }
            self.mount(path)?;
        }
        Ok(())
    }

    /// Mounts an archive (0x2c8f50 → 0x2caca8): reads its directory and puts it first in the search order.
    /// Returns false when the path is not on the disc or is not an archive.
    pub fn mount(&mut self, path: &str) -> io::Result<bool> {
        let Some(file) = self.disc.find(path).cloned() else { return Ok(false) };
        let start = file.lba as u64 * crate::disc::SECTOR as u64;
        let first = self.disc.read_at(start, (file.size as usize).min(crate::disc::SECTOR))?;
        if identify(&first) == 0 {
            // the game frees the buffer here; it never mounts a file that is not an archive
            return Ok(false);
        }
        // the first read is the start of the file; a longer directory is read again whole
        let n = header_size(&first).min(file.size as usize);
        let header = if n <= first.len() { first[..n].to_vec() } else { self.disc.read_at(start, n)? };
        self.bigs.insert(0, MountedBig { path: path.to_string(), header, file });
        Ok(true)
    }

    /// Finds a path the way the game's opener 0x2cb328 does:
    /// - `name` (no `|`): the disc (`CdFs_FindFile`); if it is not there and `search_bigs` (the open mode's
    ///   bit 0), every mounted archive, newest first;
    /// - `|name`: every mounted archive, newest first, never the disc;
    /// - `archive|name`: only the archive mounted under exactly that path (`strcmp`).
    pub fn open(&self, path: &str, search_bigs: bool) -> Option<OpenFile> {
        let (only, name) = match path.find('|') {
            None => {
                if let Some(f) = self.disc.find(path) {
                    return Some(OpenFile { start: f.lba as u64 * crate::disc::SECTOR as u64, size: f.size, big: None });
                }
                if !search_bigs {
                    return None;
                }
                (None, path)
            }
            Some(0) => (None, &path[1..]),
            Some(i) => (Some(&path[..i]), &path[i + 1..]),
        };
        for b in &self.bigs {
            if only.is_some_and(|o| o != b.path) {
                continue;
            }
            if let Some(e) = lookup(&b.header, Some(name), 0) {
                let start = b.file.lba as u64 * crate::disc::SECTOR as u64 + e.offset as u64;
                return Some(OpenFile { start, size: e.size, big: Some(b.path.clone()) });
            }
        }
        None
    }

    /// Opens, reads and unpacks a file, as the game's loader `File_LoadAndUnpack` 0x2c8938 does (its opens
    /// search the archives too).
    pub fn load(&mut self, path: &str) -> io::Result<Option<Vec<u8>>> {
        let Some(f) = self.open(path, true) else { return Ok(None) };
        let raw = self.read(&f)?;
        Ok(crate::compress::load(raw))
    }

    /// Reads an opened file's bytes (still packed if the archive packed them).
    pub fn read(&mut self, f: &OpenFile) -> io::Result<Vec<u8>> {
        self.disc.read_at(f.start, f.size as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c0fb(files: &[(&str, &[u8])]) -> Vec<u8> {
        let dir: usize = files.iter().map(|(n, _)| 6 + n.len() + 1).sum();
        let mut h = vec![0xc0, 0xfb, 0, 0];
        let hs = 6 + dir;
        h[2..4].copy_from_slice(&((hs - 4) as u16).to_be_bytes());
        h.extend_from_slice(&(files.len() as u16).to_be_bytes());
        let mut off = hs;
        let mut body = Vec::new();
        for (n, d) in files {
            h.extend_from_slice(&(off as u32).to_be_bytes()[1..]);
            h.extend_from_slice(&(d.len() as u32).to_be_bytes()[1..]);
            h.extend_from_slice(n.as_bytes());
            h.push(0);
            off += d.len();
            body.extend_from_slice(d);
        }
        h.extend(body);
        h
    }

    #[test]
    fn c0fb_lookup() {
        let big = c0fb(&[("data/a.txt", b"hello"), ("data/B.bin", b"xy")]);
        assert_eq!(identify(&big), 1);
        assert_eq!(file_data(&big, "DATA/b.BIN"), Some(&b"xy"[..]));
        assert_eq!(file_data(&big, "data/a.txt"), Some(&b"hello"[..]));
        assert_eq!(file_data(&big, "data/c"), None);
        assert_eq!(entries(&big).len(), 2);
        assert_eq!(lookup(&big, None, 1).unwrap().name, "data/B.bin");
    }

    /// Needs the player's own image: `TRICKY_ISO=... cargo test -p tricky-data big`.
    #[test]
    fn game_archives() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let mut files = Files::new(Disc::open(&iso).unwrap());
        files.mount_game_archives().unwrap();
        assert!(files.bigs.len() >= 4);
        for b in &files.bigs {
            // the C0FB count at +4 matches the directory
            if identify(&b.header) == 1 {
                assert_eq!(entries(&b.header).len(), u16::from_be_bytes([b.header[4], b.header[5]]) as usize, "{}", b.path);
            }
        }
        // a model: not on the disc by itself, found in mdlps2.big
        assert!(files.open("data/char/eddie_body.mpf", false).is_none());
        let f = files.open("data/char/eddie_body.mpf", true).expect("in a BIG");
        assert_eq!(f.big.as_deref(), Some("data/char/mdlps2.big"));
        assert_eq!(files.open("|data/char/eddie_body.mpf", false), Some(f.clone()));
        assert_eq!(files.open("data/char/mdlps2.big|data/char/eddie_body.mpf", false), Some(f.clone()));
        assert!(files.open("data/char/anm.big|data/char/eddie_body.mpf", false).is_none());
        let bytes = files.read(&f).unwrap();
        assert_eq!(bytes.len(), f.size as usize);
        assert_eq!(files.load("data/char/eddie_body.mpf").unwrap().unwrap().len(), crate::compress::unpacked_size(&bytes) as usize);
    }
}

//! The player's disc (board row E2): the ISO 9660 file table the game builds at start-up, and file
//! reads by path, from an `.iso` image or the drive itself (on Windows `\\.\E:`; both are read as 2048-byte
//! sectors).
//!
//! The game reads the disc through EA's CD file system (`lib-file`, host code on the PS2): at start-up
//! `CdFs_Mount` 0x2c9970 walks every directory from the primary volume descriptor and keeps one entry per
//! file (path hash, first sector, size, full path), sorts them by hash, and from then on finds a file by
//! hashing its upper-cased path and binary-searching that table (`CdFs_FindFile` 0x2c9cc0). This module ports
//! that table 1:1, the C library's `qsort` 0x2fb5c8 and `bsearch` 0x2fa258 included, so a path finds the same
//! file (or fails) exactly as on the PS2. Sector reads are the host's (`sceCdRead` on the PS2).

use std::cell::Cell;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// Bytes in a disc sector.
pub const SECTOR: usize = 0x800;

/// The game's maximum number of files: `CdFs_Mount`'s caller passes 1000 (0x2cc2c0's default).
pub const MAX_FILES: usize = 1000;

/// One file of the table: 12 bytes plus the path (`DAT_00349d44` = 0x100 bytes of name per entry).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscFile {
    /// `CdFs_HashPath` of `path`.
    pub hash: u32,
    /// First sector.
    pub lba: u32,
    /// Size in bytes.
    pub size: u32,
    /// Full path as the table holds it: `/DATA/CHAR/MDLPS2.BIG` (no `;1`).
    pub path: String,
}

#[derive(Debug)]
pub enum DiscError {
    Io(io::Error),
    /// More files than `MAX_FILES`: the game's mount fails.
    TooManyFiles(usize),
}

impl From<io::Error> for DiscError {
    fn from(e: io::Error) -> Self { DiscError::Io(e) }
}

impl std::fmt::Display for DiscError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscError::Io(e) => write!(f, "{e}"),
            DiscError::TooManyFiles(n) => write!(f, "{n} files on the disc, the game takes at most {MAX_FILES}"),
        }
    }
}

impl std::error::Error for DiscError {}

/// The mounted disc: the image or drive, and the game's file table.
pub struct Disc {
    src: File,
    files: Vec<DiscFile>,
    /// Two paths had the same hash: the game then sorts and searches by path instead (`DAT_00349d6c` set to
    /// `CdFs_CompareName` 0x2c9440 by `CdFs_CompareEntryHash`).
    by_name: bool,
}

impl Disc {
    /// Opens an image or a drive and builds the file table: `CdFs_Mount` 0x2c9970 (the disc is read from
    /// its root: the folder prefix at 0x3bb938 is empty).
    pub fn open(path: impl AsRef<Path>) -> Result<Disc, DiscError> {
        let mut src = File::open(path)?;
        let pvd = read_sector(&mut src, 0x10)?;
        // the root directory record at +0x9c: extent +2, size +10
        let root_lba = u32_le(&pvd, 0x9e);
        let root_size = u32_le(&pvd, 0xa6);
        let mut files = Vec::new();
        let mut prefix = String::from("/");
        scan_dir(&mut src, root_lba, (root_size + 0x7ff) >> 11, &mut prefix, &mut files)?;
        if MAX_FILES < files.len() {
            return Err(DiscError::TooManyFiles(files.len()));
        }
        let by_name = Cell::new(false);
        qsort(&mut files, |a, b| compare_entry_hash(a, b, &by_name));
        if by_name.get() {
            qsort(&mut files, |a, b| strcmp(a.path.as_bytes(), b.path.as_bytes()));
        }
        Ok(Disc { src, files, by_name: by_name.get() })
    }

    /// Every file, in the table's order.
    pub fn files(&self) -> &[DiscFile] { &self.files }

    /// Finds a file the way the game does: `CdFs_FindFile` 0x2c9cc0. A leading `/` is dropped, `\` becomes
    /// `/`, letters are upper-cased, and the key is `/` + that, so `data\char\mdlps2.big` finds
    /// `/DATA/CHAR/MDLPS2.BIG`.
    pub fn find(&self, path: &str) -> Option<&DiscFile> {
        let mut key = vec![b'/'];
        let p = path.as_bytes();
        let p = if p.first() == Some(&b'/') { &p[1..] } else { p };
        for &c in p {
            key.push(if c == b'\\' { b'/' } else { c.to_ascii_uppercase() });
        }
        let i = if self.by_name {
            bsearch(&self.files, |e| strcmp(&key, e.path.as_bytes()))
        } else {
            let h = hash_path(&key);
            bsearch(&self.files, |e| h.wrapping_sub(e.hash) as i32)
        }?;
        Some(&self.files[i])
    }

    /// Reads a whole file. Host code: the game streams sectors with `sceCdRead`.
    pub fn read(&mut self, file: &DiscFile) -> io::Result<Vec<u8>> {
        let size = file.size as usize;
        self.src.seek(SeekFrom::Start(file.lba as u64 * SECTOR as u64))?;
        // drives only take whole sectors
        let mut buf = vec![0; (size + SECTOR - 1) / SECTOR * SECTOR];
        self.src.read_exact(&mut buf)?;
        buf.truncate(size);
        Ok(buf)
    }

    /// Reads `size` bytes from byte `start` of the disc (whole sectors, as drives need).
    pub fn read_at(&mut self, start: u64, size: usize) -> io::Result<Vec<u8>> {
        let first = start / SECTOR as u64;
        let skip = (start % SECTOR as u64) as usize;
        self.src.seek(SeekFrom::Start(first * SECTOR as u64))?;
        let mut buf = vec![0; (skip + size + SECTOR - 1) / SECTOR * SECTOR];
        self.src.read_exact(&mut buf)?;
        Ok(buf[skip..skip + size].to_vec())
    }

    /// `find` then `read`.
    pub fn read_path(&mut self, path: &str) -> io::Result<Option<Vec<u8>>> {
        match self.find(path).cloned() {
            Some(f) => self.read(&f).map(Some),
            None => Ok(None),
        }
    }
}

fn read_sector(src: &mut File, lba: u32) -> io::Result<[u8; SECTOR]> {
    let mut s = [0; SECTOR];
    src.seek(SeekFrom::Start(lba as u64 * SECTOR as u64))?;
    src.read_exact(&mut s)?;
    Ok(s)
}

fn u32_le(b: &[u8], at: usize) -> u32 { u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) }

/// `CdFs_ScanDir` 0x2c9200: one directory, `sectors` long, from `lba`. The first two records (`.` and `..`)
/// are skipped; a zero length byte ends the records of a sector. A subdirectory is walked at once with
/// `name/` added to the prefix. A file's name loses its last two characters (`;1`) and then a trailing `.`.
fn scan_dir(src: &mut File, lba: u32, sectors: u32, prefix: &mut String, out: &mut Vec<DiscFile>) -> io::Result<()> {
    let mut lba = lba;
    let mut left = sectors - 1;
    let mut s = read_sector(src, lba)?;
    let mut at = s[0] as usize;
    loop {
        at += s[at] as usize;
        while at >= SECTOR || s[at] == 0 {
            if left == 0 {
                return Ok(());
            }
            left -= 1;
            lba += 1;
            s = read_sector(src, lba)?;
            at = 0;
        }
        let r = &s[at..];
        let name_len = r[0x20] as usize;
        let name = &r[0x21..0x21 + name_len];
        if r[0x19] & 2 != 0 {
            let saved = prefix.len();
            prefix.push_str(&String::from_utf8_lossy(name));
            prefix.push('/');
            scan_dir(src, u32_le(r, 2), (u32_le(r, 10) + 0x7ff) >> 11, prefix, out)?;
            prefix.truncate(saved);
            continue;
        }
        let mut n = &name[..name_len.saturating_sub(2)];
        if n.last() == Some(&b'.') {
            n = &n[..n.len() - 1];
        }
        let path = format!("{prefix}{}", String::from_utf8_lossy(n));
        out.push(DiscFile { hash: hash_path(path.as_bytes()), lba: u32_le(r, 2), size: u32_le(r, 10), path });
    }
}

/// `CdFs_HashPath` 0x2c9190: an ELF-style hash taken from the end of the string back to its second
/// character (the leading `/` is left out; the terminating zero is the first, null, step).
pub fn hash_path(s: &[u8]) -> u32 {
    let len = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    let at = |i: usize| if i < len { s[i] } else { 0 };
    let mut h: u32 = 0;
    let mut p = len;
    if p != 0 {
        let mut c = at(p);
        loop {
            h = h.wrapping_mul(16).wrapping_add(c as u32);
            let g = h & 0xf000_0000;
            p -= 1;
            if g != 0 {
                h ^= g >> 24 ^ g;
            }
            if p == 0 {
                break;
            }
            c = at(p);
        }
    }
    h
}

/// `CdFs_CompareEntryHash` 0x2c9460: the 32-bit difference of the hashes; equal hashes switch the table to
/// sorting and searching by path.
fn compare_entry_hash(a: &DiscFile, b: &DiscFile, by_name: &Cell<bool>) -> i32 {
    let d = a.hash.wrapping_sub(b.hash) as i32;
    if d == 0 {
        by_name.set(true);
    }
    d
}

/// `strcmp` 0x2fd008 (`CdFs_CompareName` 0x2c9440, `CdFs_CompareEntryName` 0x2c9420): sign of the first
/// differing byte.
fn strcmp(a: &[u8], b: &[u8]) -> i32 {
    let mut i = 0;
    loop {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y || x == 0 {
            return x as i32 - y as i32;
        }
        i += 1;
    }
}

/// `bsearch` 0x2fa258, over the elements: the index of one the key compares equal to.
fn bsearch<T>(v: &[T], mut cmp: impl FnMut(&T) -> i32) -> Option<usize> {
    let (mut lo, mut hi) = (0usize, v.len());
    while lo < hi {
        let mid = (lo + hi) >> 1;
        let r = cmp(&v[mid]);
        if r < 0 {
            hi = mid;
        } else if r == 0 {
            return Some(mid);
        } else {
            lo = mid + 1;
        }
    }
    None
}

/// The C library's `qsort` 0x2fb5c8, which is Bentley and McIlroy's: insertion sort below 7 elements, the
/// median of three (of three medians above 40), three-way partition with the equal keys swapped to both
/// ends, insertion sort when a pass swapped nothing, a call on the left part and a loop on the right one.
/// The comparison is called in the same order as the game's, so a comparison that is not a true order (the
/// wrapping hash difference) leaves the same order behind.
pub fn qsort<T>(v: &mut [T], mut cmp: impl FnMut(&T, &T) -> i32) {
    qsort_part(v, 0, v.len(), &mut cmp);
}

fn qsort_part<T>(v: &mut [T], mut a: usize, mut n: usize, cmp: &mut impl FnMut(&T, &T) -> i32) {
    loop {
        let mut swapped = false;
        if n < 7 {
            insertion(v, a, n, cmp);
            return;
        }
        let mut pm = a + n / 2;
        if n > 7 {
            let mut pl = a;
            let mut pn = a + n - 1;
            if n > 40 {
                let d = n / 8;
                pl = med3(v, pl, pl + d, pl + 2 * d, cmp);
                pm = med3(v, pm - d, pm, pm + d, cmp);
                pn = med3(v, pn - 2 * d, pn - d, pn, cmp);
            }
            pm = med3(v, pl, pm, pn, cmp);
        }
        v.swap(a, pm);
        let (mut pa, mut pb) = (a + 1, a + 1);
        let (mut pc, mut pd) = (a + n - 1, a + n - 1);
        loop {
            while pb <= pc {
                let r = cmp(&v[pb], &v[a]);
                if r > 0 {
                    break;
                }
                if r == 0 {
                    swapped = true;
                    v.swap(pa, pb);
                    pa += 1;
                }
                pb += 1;
            }
            while pb <= pc {
                let r = cmp(&v[pc], &v[a]);
                if r < 0 {
                    break;
                }
                if r == 0 {
                    swapped = true;
                    v.swap(pc, pd);
                    pd -= 1;
                }
                pc -= 1;
            }
            if pb > pc {
                break;
            }
            v.swap(pb, pc);
            swapped = true;
            pb += 1;
            pc -= 1;
        }
        if !swapped {
            insertion(v, a, n, cmp);
            return;
        }
        let pn = a + n;
        let r = (pa - a).min(pb - pa);
        vecswap(v, a, pb - r, r);
        let r = (pd - pc).min(pn - pd - 1);
        vecswap(v, pb, pn - r, r);
        let r = pb - pa;
        if r > 1 {
            qsort_part(v, a, r, cmp);
        }
        let r = pd - pc;
        if r <= 1 {
            return;
        }
        a = pn - r;
        n = r;
    }
}

fn insertion<T>(v: &mut [T], a: usize, n: usize, cmp: &mut impl FnMut(&T, &T) -> i32) {
    for pm in a + 1..a + n {
        let mut pl = pm;
        while pl > a && cmp(&v[pl - 1], &v[pl]) > 0 {
            v.swap(pl, pl - 1);
            pl -= 1;
        }
    }
}

fn med3<T>(v: &[T], a: usize, b: usize, c: usize, cmp: &mut impl FnMut(&T, &T) -> i32) -> usize {
    if cmp(&v[a], &v[b]) < 0 {
        if cmp(&v[b], &v[c]) < 0 { b } else if cmp(&v[a], &v[c]) < 0 { c } else { a }
    } else if cmp(&v[b], &v[c]) > 0 {
        b
    } else if cmp(&v[a], &v[c]) < 0 {
        a
    } else {
        c
    }
}

fn vecswap<T>(v: &mut [T], a: usize, b: usize, n: usize) {
    for i in 0..n {
        v.swap(a + i, b + i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_skips_the_first_character() {
        // h = ((('C')*16 + 'B')... from the end: "/AB" hashes 'B' then 'A'
        assert_eq!(hash_path(b"/AB"), (b'B' as u32) * 16 + b'A' as u32);
        assert_eq!(hash_path(b"/AB"), hash_path(b"XAB"));
        assert_eq!(hash_path(b""), 0);
    }

    #[test]
    fn qsort_sorts() {
        let mut seed = 12345u32;
        for n in [0, 1, 5, 7, 8, 40, 41, 300] {
            let mut v: Vec<u32> = (0..n).map(|_| { seed = seed.wrapping_mul(1103515245).wrapping_add(12345); seed % 50 }).collect();
            let mut w = v.clone();
            qsort(&mut v, |a, b| *a as i32 - *b as i32);
            w.sort();
            assert_eq!(v, w);
        }
    }

    /// Needs the player's own image: `TRICKY_ISO=path/to/SSX Tricky.iso cargo test -p tricky-data disc`.
    #[test]
    fn mounts_the_players_disc() {
        let Ok(iso) = std::env::var("TRICKY_ISO") else { return };
        let disc = Disc::open(&iso).expect("mount");
        assert!(!disc.files().is_empty());
        // every file is found under its own path, in either spelling
        for f in disc.files() {
            assert_eq!(disc.find(&f.path).map(|x| &x.path), Some(&f.path));
            assert_eq!(disc.find(&f.path[1..].to_ascii_lowercase().replace('/', "\\")).map(|x| &x.path), Some(&f.path));
        }
        assert!(disc.find("data/char/mdlps2.big").is_some());
        assert!(disc.find("data/char/nosuchfile.big").is_none());
    }
}

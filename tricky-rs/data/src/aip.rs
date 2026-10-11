//! A course's `.aip` and `.sop` files (board row E3d), read as `AIP_LoadPathsFromFile` 0x197da0 reads them: the
//! computer riders' path network (`cAIPath`, section 0) and the race lines with their events (`cEventPath`, section
//! 1). Little-endian words throughout; checked against the game's own loader in the function runner
//! (`tricky-game/tests/aip.rs`).
//!
//! File: a word the loader skips (0x0a0a0a0a), the section count, then sections of (type, size, data).
//! AI paths (`AIP_LoadAIPaths` 0x198518): path count, start-position count, the start positions (path indices),
//! the paths. Event paths (`AIP_LoadEventPaths` 0x1983d0): path count, the main path's index, the paths.
//! A path (`AIPath_Load` 0x196f28): parameters (count, then id, size, bytes: `cAIPath` 100 its preference U3
//! (default 50), 101 respawnable (default 1); `cEventPath` 0 its distance to the finish at the start), the point and
//! event counts, three vectors (the start, then two more: the box the path fits in), points of 16 bytes (the offset
//! from the last and a distance) and events of 16 bytes (type, value, start, end; the type stored as its place in
//! the table at 0x337e70).

use std::fmt;

/// The event types a file may use, in the order the game stores them (0x337e70): a type is kept as its place.
pub const EVENT_TYPES: [i32; 32] = [-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 100, 101, 102, 103, 104, 105, 300];

/// One event on a path: its type as in the file (`code`) and as the game keeps it (`kind`, the place in
/// [`EVENT_TYPES`]; 0 for a type not in the table), its value and the stretch of path it covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AipEvent {
    pub code: i32,
    pub kind: u32,
    pub value: u32,
    pub start: f32,
    pub end: f32,
}

/// One path: its parameters as (id, bytes), the three vectors after the counts (`start`, then `v1`, `v2`), its
/// points (offset, distance) and events.
#[derive(Clone, Debug, PartialEq)]
pub struct AipPath {
    pub params: Vec<(u32, Vec<u8>)>,
    pub start: [f32; 3],
    pub v1: [f32; 3],
    pub v2: [f32; 3],
    pub points: Vec<([f32; 3], f32)>,
    pub events: Vec<AipEvent>,
}

impl AipPath {
    /// A parameter's first word, if the path has it.
    pub fn param(&self, id: u32) -> Option<u32> {
        self.params.iter().find(|p| p.0 == id).and_then(|p| p.1.get(..4)).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// `cAIPath`'s preference (+0x38): parameter 100, else 50.
    pub fn u3(&self) -> u32 {
        self.param(100).unwrap_or(50)
    }
    /// `cAIPath`'s respawnable flag (+0x3c): parameter 101, else 1.
    pub fn respawnable(&self) -> u32 {
        self.param(101).unwrap_or(1)
    }
}

/// A whole file: the AI paths with the start positions, the event paths with the main one's index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Aip {
    pub start_positions: Vec<u32>,
    pub ai_paths: Vec<AipPath>,
    pub main_event_path: u32,
    pub event_paths: Vec<AipPath>,
}

#[derive(Debug)]
pub struct AipError(pub String);
impl fmt::Display for AipError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "aip: {}", self.0)
    }
}

struct Rd<'a> {
    b: &'a [u8],
    at: usize,
}

impl Rd<'_> {
    fn u32(&mut self) -> Result<u32, AipError> {
        let s = self.b.get(self.at..self.at + 4).ok_or_else(|| AipError(format!("ends at {:#x}", self.at)))?;
        self.at += 4;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f32(&mut self) -> Result<f32, AipError> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn v3(&mut self) -> Result<[f32; 3], AipError> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    fn bytes(&mut self, n: usize) -> Result<Vec<u8>, AipError> {
        let s = self.b.get(self.at..self.at + n).ok_or_else(|| AipError(format!("ends at {:#x}", self.at)))?;
        self.at += n;
        Ok(s.to_vec())
    }
}

fn path(r: &mut Rd) -> Result<AipPath, AipError> {
    let n = r.u32()? as i32;
    let mut params = Vec::new();
    for _ in 0..n.max(0) {
        let id = r.u32()?;
        let size = r.u32()? as usize;
        params.push((id, r.bytes(size)?));
    }
    let points = r.u32()? as i32;
    let events = r.u32()? as i32;
    let start = r.v3()?;
    let v1 = r.v3()?;
    let v2 = r.v3()?;
    let mut p = AipPath { params, start, v1, v2, points: Vec::new(), events: Vec::new() };
    for _ in 0..points.max(0) {
        p.points.push((r.v3()?, r.f32()?));
    }
    for _ in 0..events.max(0) {
        let code = r.u32()? as i32;
        let kind = EVENT_TYPES.iter().position(|t| *t == code).unwrap_or(0) as u32;
        p.events.push(AipEvent { code, kind, value: r.u32()?, start: r.f32()?, end: r.f32()? });
    }
    Ok(p)
}

/// Reads a `.aip` or `.sop` file (unpacked).
pub fn read(b: &[u8]) -> Result<Aip, AipError> {
    let mut r = Rd { b, at: 4 };
    let sections = r.u32()? as i32;
    let mut out = Aip::default();
    for _ in 0..sections.max(0) {
        let kind = r.u32()?;
        let size = r.u32()? as usize;
        let end = r.at + size;
        match kind {
            0 => {
                let n = r.u32()? as i32;
                let starts = r.u32()? as i32;
                for _ in 0..starts.max(0) {
                    out.start_positions.push(r.u32()?);
                }
                for _ in 0..n.max(0) {
                    out.ai_paths.push(path(&mut r)?);
                }
            }
            1 => {
                let n = r.u32()? as i32;
                out.main_event_path = r.u32()?;
                for _ in 0..n.max(0) {
                    out.event_paths.push(path(&mut r)?);
                }
            }
            _ => {}
        }
        r.at = end;
    }
    Ok(out)
}

/// The course's archive name on the disc (DATA/MODELS/<name>.BIG) for a level folder's name.
pub fn big_name(folder: &str) -> Option<&'static str> {
    Some(match folder {
        "gari" => "gari",
        "snowdream" => "snow",
        "elysium" => "elysium",
        "mesablanca" => "mesa",
        "merqury" => "merquer",
        "aloha" => "aloha",
        "pipedream" => "pipe",
        "untracked" => "untrack",
        "megaplex" => "megaple",
        "trick" => "trick",
        "alaska" => "alaska",
        _ => return None,
    })
}

/// A course's `.aip` (`ext` "aip") or `.sop` ("sop") from the player's disc: DATA/MODELS/<course>.BIG, unpacked.
pub fn from_disc(files: &mut crate::big::Files, folder: &str, ext: &str) -> Option<Aip> {
    let name = big_name(folder)?;
    let big = files.load(&format!("data/models/{name}.big")).ok().flatten()?;
    let entry = crate::big::entries(&big).into_iter().find(|e| e.name.eq_ignore_ascii_case(&format!("Data/models/{name}.{ext}")))?;
    let data = crate::big::file_data(&big, &entry.name)?;
    let un = crate::compress::load(data.to_vec()).unwrap_or_else(|| data.to_vec());
    read(&un).ok()
}

//! A song on Pathfinder as the game plays it (board row G10b2c1): the graph registered and the track loaded as
//! `Music_LoadPathfinder` 0x223d60 calls `PathX_RegisterSong` and `PathX_LoadSong` (song 0, track 0, latency 500, the
//! stream settings record, a ring of 2, 0x96, the track record in the song's 0x37000-byte buffer), the events posted
//! as `Song_MusicEvent` does (priority 0), the path variable as `Song_SetPathVar`, and `update` run as the game's poll
//! task runs it, over [`Driver`].
//!
//! STANDIN: [`Driver`] is the port's own stream driver for the PS2's (`StreamSlot_*` 0x223158..0x223c40 over EA's
//! `SndStream_*`, the IOP's streaming), modelled on what the EE side asks of it: `StreamSlot_Play` queues a request
//! at a segment's offset (purging the stream first for an immediate one), held when its start is −1 until
//! `FUN_00223a90` releases the last one; `StreamSlot_IsDone` is true with fewer than two requests and the one there
//! read in full (the next can be queued; the engine then queues it once the one playing is past the latency); `StreamSlot_GetPosition` / `PathX_GetPositionMs` the request playing, its ms played and its length;
//! `PathX_TimeMs` the clock. Requests play back to back, sample-exact; the audio thread pulls the frames
//! ([`Driver::pull`]) and the clock is the frames played.

use std::collections::{HashMap, VecDeque};

use crate::pf::{Mem, Pf, PfDriver, PfEnv, AUTHOR, GLOBALS};

/// Where the engine's memory is: its globals and song records, the authoring string, the graph, the track's buffer,
/// the file's name and the stream settings
pub const GLOBALS_AT: u32 = 0x349500;
pub const AUTHOR_TEXT: u32 = 0x349e00;
pub const GRAPH: u32 = 0x0100_0000;
pub const GRAPH_SIZE: u32 = 0x7000;
pub const TRACK_BUFFER: u32 = 0x0101_0000;
pub const TRACK_BUFFER_SIZE: u32 = 0x37000;
pub const MISC: u32 = 0x0105_0000;
const NAME_AT: u32 = MISC;
const SETTINGS: u32 = MISC + 0x100;

/// One queued request: a segment's frames in the decoded audio
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    pub handle: i32,
    pub start: usize,
    pub len: usize,
    pub held: bool,
    pub volume: u8,
}

/// The stream driver (STANDIN, see the module): one stream slot, its requests and its clock
#[derive(Debug, Default)]
pub struct Driver {
    pub rate: u32,
    /// the segments' frames by their offset in the song's audio (`segment_offset`)
    pub segments: HashMap<u32, (usize, usize)>,
    pub queue: VecDeque<Request>,
    /// the request playing and the frames of it played
    pub cur: Option<(Request, usize)>,
    pub frames: u64,
    next_handle: i32,
    /// the requests started, in order (for checks)
    pub started: Vec<(u64, u32)>,
    offsets: HashMap<i32, u32>,
    pub stopped: bool,
    /// the stream's volume (`SndStream_SetVolume`, 0..127), paused (`SndStream_SetPitch` 0), a fade out (the volume
    /// it started from, the frames it lasts, the frames done)
    pub volume: u8,
    pub paused: bool,
    pub fade: Option<(u8, u64, u64)>,
}

impl Driver {
    pub fn new(rate: u32, segments: HashMap<u32, (usize, usize)>) -> Driver {
        Driver { rate, segments, next_handle: 1, volume: 0x7f, ..Default::default() }
    }
    fn ms(&self, frames: usize) -> i32 {
        (frames as u64 * 1000 / self.rate.max(1) as u64) as i32
    }
    /// The frames the audio thread plays next: (where in the audio, how many, the volume); `None` for silence. The
    /// clock moves by `n` frames either way
    pub fn pull(&mut self, n: usize) -> Vec<(usize, usize, u8)> {
        let mut out = Vec::new();
        if self.paused {
            return out;
        }
        // the stream's volume over the request's: the fade's, else the stream's
        let stream = match &mut self.fade {
            Some((from, len, done)) => {
                *done += n as u64;
                let v = (*from as u64 * len.saturating_sub(*done) / (*len).max(1)) as u8;
                if *done >= *len {
                    self.queue.clear();
                    self.cur = None;
                }
                v
            }
            None => self.volume,
        };
        let mut left = n;
        while left > 0 {
            if self.cur.is_none() {
                match self.queue.front() {
                    Some(r) if !r.held => {
                        let r = self.queue.pop_front().unwrap();
                        let off = self.offsets.get(&r.handle).copied().unwrap_or(0);
                        self.started.push((self.frames + (n - left) as u64, off));
                        self.cur = Some((r, 0));
                    }
                    _ => break,
                }
            }
            let (r, pos) = self.cur.unwrap();
            let k = left.min(r.len - pos);
            let v = if r.volume <= 0x7f { (r.volume as u32 * stream as u32 / 0x7f) as u8 } else { stream };
            out.push((r.start + pos, k, v));
            left -= k;
            if pos + k >= r.len {
                self.cur = None;
            } else {
                self.cur = Some((r, pos + k));
            }
        }
        self.frames += n as u64;
        out
    }
}

impl Driver {
    /// `SndStream_FadeVolume` to 0 over `ms`
    pub fn fade_out(&mut self, ms: i32) {
        self.fade = Some((self.volume, ms.max(1) as u64 * self.rate as u64 / 1000, 0));
    }
    /// `SndStream_GetStatus`'s first two words: the requests on the stream, the handle playing (−1 none)
    pub fn requests(&self) -> usize {
        self.queue.len() + self.cur.is_some() as usize
    }
    pub fn handle_playing(&self) -> i32 {
        self.cur.map_or(self.queue.front().map_or(-1, |r| r.handle), |c| c.0.handle)
    }
    /// `SndHandle_GetState`: (2 playing with its ms played and its length; 1 waiting; 3 done)
    pub fn handle_state(&self, h: i32) -> (u32, i32, i32) {
        match self.cur {
            Some((r, p)) if r.handle == h => (2, self.ms(p), self.ms(r.len)),
            _ if self.queue.iter().any(|r| r.handle == h) => (1, 0, 0),
            _ => (3, 0, 0),
        }
    }
}

impl PfEnv for Driver {
    fn time_ms(&mut self) -> u32 {
        (self.frames * 1000 / self.rate.max(1) as u64) as u32
    }
    fn position_ms(&mut self, _slot: u32) -> i32 {
        self.cur.map_or(0, |(_, p)| self.ms(p))
    }
    fn play(&mut self, _slot: u32, start: i64, _name: u32, offset: u32, volume: u8, _beat: u8, immediate: u8) -> i32 {
        if immediate != 0 {
            self.queue.clear();
            self.cur = None;
        }
        let Some(&(s, l)) = self.segments.get(&offset) else { return -1 };
        let h = self.next_handle;
        self.next_handle += 1;
        self.offsets.insert(h, offset);
        self.queue.push_back(Request { handle: h, start: s, len: l, held: start < 0, volume });
        h
    }
    fn stop(&mut self, _slot: u32) {
        self.queue.clear();
        self.cur = None;
    }
    fn fade(&mut self, _slot: u32) {
        self.queue.clear();
        self.cur = None;
    }
    fn set_volume(&mut self, _slot: u32, _volume: i32, _ms: Option<i32>) {}
    /// `StreamSlot_IsDone`: fewer than two requests on the stream and the one there read in full (a request playing
    /// here; the driver holds the decoded audio, so reading is never behind), or none at all; a request waiting to
    /// start is not
    fn is_done(&mut self, _slot: u32) -> bool {
        match (self.cur.is_some(), self.queue.len()) {
            (false, 0) => true,
            (true, 0) => true,
            _ => false,
        }
    }
    fn get_position(&mut self, _slot: u32) -> (i32, i32, i32) {
        match self.cur {
            Some((r, p)) => (r.handle, self.ms(p), self.ms(r.len)),
            None => (-1, -1, -1),
        }
    }
    fn release(&mut self, _slot: u32) {
        if let Some(r) = self.queue.back_mut() {
            r.held = false;
        }
    }
}

impl PfDriver for Driver {
    fn free_track(&mut self, slot: u32) {
        self.stop(slot);
    }
    fn stop_driver(&mut self) {
        self.stopped = true;
    }
    fn set_pitch(&mut self, _slot: u32, _pitch: u8) {}
    fn fade_to(&mut self, _slot: u32, _volume: i32, _ms: u64) {}
    fn track_overhead(&mut self) -> i32 {
        0
    }
    fn init_driver(&mut self) {}
    fn open_song(&mut self, _name: u32, _a: u32, _n: i32, _b: u32, _buffer: u32, _size: i32) -> i32 {
        0
    }
}

/// A song playing on Pathfinder: the engine's memory (an engine is made over it for each call, with no marker
/// callback, so the player can live on any thread)
pub struct Player {
    pub mem: Mem,
    /// the track's id (`PathX_LoadSong`'s answer: the song's bit 24 and the track's)
    pub id: u32,
}

impl Player {
    /// The engine's memory, the graph (the `.mpf`'s bytes) registered and the track loaded (as `Music_LoadPathfinder`
    /// does it); then the driver's segments mapped from the graph's offsets (`segments` by segment number: frames).
    /// `None` when the graph does not register or the track does not load
    pub fn load(graph: &[u8], name: &[u8], segments: &HashMap<i16, (usize, usize)>, d: &mut Driver) -> Option<Player> {
        let mut mem = Mem::default();
        mem.regions.push((GLOBALS_AT, vec![0; 0xb00]));
        let mut g = graph.to_vec();
        g.resize(GRAPH_SIZE.max(graph.len() as u32) as usize, 0);
        mem.regions.push((GRAPH, g));
        mem.regions.push((TRACK_BUFFER, vec![0; TRACK_BUFFER_SIZE as usize]));
        mem.regions.push((MISC, vec![0; 0x200]));
        mem.w32(AUTHOR, AUTHOR_TEXT);
        for (k, b) in name.iter().enumerate() {
            mem.w8(NAME_AT + k as u32, *b);
        }
        // no song selected (the byte starts at 0xff in the game's data), then `Path_Init`
        mem.w8(GLOBALS + 0xb, 0xff);
        let mut pf = Pf { mem, marker: None };
        pf.init();
        if pf.register(GRAPH) < 0 {
            return None;
        }
        let id = pf.load_song(d, 0, 0, NAME_AT, name, 500, SETTINGS, 2, 0x96, TRACK_BUFFER, TRACK_BUFFER_SIZE as i32);
        if id < 0 {
            return None;
        }
        // the segments by the offsets the engine will ask for
        if 0 <= pf.select(0) {
            for n in 0..pf.node_count() {
                let s = pf.node_segment(n);
                if 0 < s {
                    if let Some(&f) = segments.get(&s) {
                        d.segments.insert(pf.segment_offset(n as i64), f);
                    }
                }
            }
        }
        pf.mem.w32(GLOBALS, 0);
        pf.mem.w8(GLOBALS + 0xb, 0xff);
        Some(Player { mem: pf.mem, id: id as u32 })
    }

    fn with<R>(&mut self, f: impl FnOnce(&mut Pf, u32) -> R) -> R {
        let mut pf = Pf { mem: std::mem::take(&mut self.mem), marker: None };
        let r = f(&mut pf, self.id);
        self.mem = pf.mem;
        r
    }

    /// `Song_MusicEvent`: the event posted to the track, priority 0
    pub fn event(&mut self, d: &mut Driver, event: i16) -> i32 {
        self.with(|pf, id| pf.post_event(d, id, event, 0))
    }

    /// `Song_SetPathVar`: the path variable
    pub fn set_var(&mut self, var: u8) -> i32 {
        self.with(|pf, id| pf.set_variable(id, var as u64))
    }

    /// The driver's update, as the game's poll task runs it
    pub fn update(&mut self, d: &mut Driver) {
        self.with(|pf, _| pf.update(d));
    }

    /// `PathX_StopTracks`
    pub fn stop(&mut self, d: &mut Driver) {
        self.with(|pf, id| pf.stop_tracks(d, id));
    }
}

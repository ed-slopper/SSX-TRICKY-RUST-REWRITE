//! EA's Pathfinder interactive music as the game runs it (board row G10b; `PathX_*` / `Path_*` 0x2bec00..0x2c2040).
//! A song is its `.mpf` graph ("PFDx", version 0x103: nodes, branches by a path variable, routers, events) and up to
//! 24 tracks playing through it; four songs can be loaded. The engine works on the graph and the track records in
//! memory through pointers, so the port keeps the same bytes at the same addresses ([`Mem`]: the song table at
//! 0x3495c8 and wherever the caller put the graph and the tracks) and the game's pointer arithmetic on them.
//! Part 1: the graph's walk (`FUN_002bf038` the branch by the variable, `FUN_002bf290` the router, `FUN_002bf108`
//! through marker nodes, `FUN_002bf320` / `FUN_002bf490` the event table, `Path_Init`, `PathX_SelectSong`,
//! `PathX_RegisterSong`). Checked in the function runner (`tests/pf.rs`).

/// The globals: the selected song's record (0x3495c8), the marker callback (0x3495cc), started (0x3495d0), paused
/// (0x3495d1), +0x3495d2, the selected song (0x3495d3, −1 none); then the four song records from 0x3495e0, 0x1d4
/// each (+0x14 the graph, +0x174 the 24 tracks)
pub const GLOBALS: u32 = 0x3495c8;
pub const SONGS: u32 = 0x3495e0;
pub const SONG_SIZE: u32 = 0x1d4;
/// The pointer to the authoring string ("PATHAUTH Barton Jaques ...", 0x349d30) whose first byte `Path_Init` sets
pub const AUTHOR: u32 = 0x349d30;

/// Memory as the engine sees it: regions of bytes at their addresses
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mem {
    pub regions: Vec<(u32, Vec<u8>)>,
}

impl Mem {
    fn at(&self, a: u32, n: u32) -> (usize, usize) {
        for (k, (base, v)) in self.regions.iter().enumerate() {
            if *base <= a && a + n <= base + v.len() as u32 {
                return (k, (a - base) as usize);
            }
        }
        panic!("pathfinder: {a:#x} is outside the memory given");
    }
    pub fn r8(&self, a: u32) -> u8 {
        let (k, o) = self.at(a, 1);
        self.regions[k].1[o]
    }
    pub fn r16(&self, a: u32) -> u16 {
        let (k, o) = self.at(a, 2);
        u16::from_le_bytes(self.regions[k].1[o..o + 2].try_into().unwrap())
    }
    pub fn r32(&self, a: u32) -> u32 {
        let (k, o) = self.at(a, 4);
        u32::from_le_bytes(self.regions[k].1[o..o + 4].try_into().unwrap())
    }
    pub fn w8(&mut self, a: u32, v: u8) {
        let (k, o) = self.at(a, 1);
        self.regions[k].1[o] = v;
    }
    pub fn w16(&mut self, a: u32, v: u16) {
        let (k, o) = self.at(a, 2);
        self.regions[k].1[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
    pub fn w32(&mut self, a: u32, v: u32) {
        let (k, o) = self.at(a, 4);
        self.regions[k].1[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn s16(&self, a: u32) -> i16 {
        self.r16(a) as i16
    }
}

/// The engine: its memory and the marker callback (`DAT_003495cc`: called with the song and the node when the walk
/// passes a marker node; none when null)
pub struct Pf<'a> {
    pub mem: Mem,
    pub marker: Option<&'a mut dyn FnMut(i8, i16)>,
}

impl Pf<'_> {
    /// The selected song's record (`DAT_003495c8`)
    pub fn cur(&self) -> u32 {
        self.mem.r32(GLOBALS)
    }
    fn song_index(&self) -> i8 {
        self.mem.r8(GLOBALS + 0xb) as i8
    }
    /// The graph's base (+0x14)
    fn graph(&self) -> u32 {
        self.mem.r32(self.cur() + 0x14)
    }
    /// A node's record: the graph plus the node's word offset (the table of shorts at +0x18)
    fn node(&self, n: i32) -> u32 {
        let off = self.mem.s16(self.mem.r32(self.cur() + 0x18).wrapping_add((n * 2) as u32)) as i32;
        self.graph().wrapping_add((off * 4) as u32)
    }
    /// Track k's record (+0x174 + 4k)
    fn track(&self, k: u32) -> u32 {
        self.mem.r32(self.cur() + 0x174 + 4 * k)
    }

    /// `Path_Init` 0x2beca0: every song's graph and tracks cleared, none selected, started, not paused; the authoring
    /// string's first byte 'P'. (Refused, −0x12, while a song is selected.)
    pub fn init(&mut self) -> i32 {
        if 0 <= self.song_index() {
            return -0x12;
        }
        for i in 0..4 {
            let s = SONGS + i * SONG_SIZE;
            self.mem.w32(s + 0x14, 0);
            for k in 0..24 {
                self.mem.w32(s + 0x174 + 4 * k, 0);
            }
        }
        self.mem.w8(GLOBALS + 0xb, 0xff);
        self.mem.w8(GLOBALS + 8, 1);
        self.mem.w32(GLOBALS, 0);
        self.mem.w8(GLOBALS + 9, 0);
        let auth = self.mem.r32(AUTHOR);
        self.mem.w8(auth, b'P');
        0
    }

    /// `PathX_SelectSong` 0x2bed58: a loaded song (0..3) becomes the selected one; else none (−1)
    pub fn select(&mut self, song: u64) -> i64 {
        if song < 4 && self.mem.r32(SONGS + song as u32 * SONG_SIZE + 0x14) != 0 {
            self.mem.w32(GLOBALS, SONGS + song as u32 * SONG_SIZE);
            self.mem.w8(GLOBALS + 0xb, song as u8);
            return song as i64;
        }
        self.mem.w8(GLOBALS + 0xb, 0xff);
        -1
    }

    /// `PathX_RegisterSong` 0x2c0b08(graph at `at`): the song slot the graph names (+0xc) takes it when free and the
    /// graph is "PFDx" version 0x103: the record's tables found (+0x18 nodes, +0x1c, +0x20 the event grid, +0x24 its
    /// entries, +0x28 the routers, +0x2c, +0x30), +8 = 50, +0xc = 10, +0x10, +0x44, +0x58 cleared; the slot (or −1;
    /// −0x12 while a song is selected)
    pub fn register(&mut self, at: u32) -> i64 {
        if self.mem.r8(GLOBALS + 8) == 0 {
            self.init();
        }
        if 0 <= self.song_index() {
            return -0x12;
        }
        let mut out = -1i64;
        let slot = self.mem.r8(at + 0xc) as u32;
        let rec = SONGS + slot * SONG_SIZE;
        if self.mem.r32(rec + 0x14) == 0 && self.mem.r32(at) == 0x5046_4478 && self.mem.r16(at + 4) == 0x103 {
            self.mem.w32(rec + 0x14, at);
            self.select(slot as u64);
            let c = self.cur();
            let g = self.mem.r32(c + 0x14);
            let nodes = g + 0x24;
            self.mem.w32(c + 0x18, nodes);
            let n = self.mem.r16(g + 0x12) as u32;
            self.mem.w32(c + 0x1c, nodes + n * 2);
            let (b2, b3, b4) = (self.mem.r8(g + 0xe) as u32, self.mem.r8(g + 0xf) as u32, self.mem.r8(g + 0xd) as u32);
            let last = g.wrapping_add((self.mem.s16(n * 2 + nodes - 2) as i32 * 4) as u32);
            let grid = last + 0xc + self.mem.r8(last + 0xb) as u32 * 4;
            self.mem.w32(c + 0x20, grid);
            let entries = grid + ((b3 * b2 * b4 + 3) & !3);
            self.mem.w32(c + 0x24, entries);
            let routers = entries + self.mem.r8(g + 0x10) as u32 * 4;
            self.mem.w32(c + 0x28, routers);
            let x2c = g.wrapping_add(self.mem.r32(self.mem.r8(g + 0x11) as u32 * 4 + routers).wrapping_mul(4));
            self.mem.w32(c + 0x2c, x2c);
            let tracks = self.mem.r8(g + 0xd) as u32;
            self.mem.w32(c + 8, 0x32);
            self.mem.w32(c + 0xc, 10);
            self.mem.w8(c + 0x10, 0);
            self.mem.w32(c + 0x30, x2c + tracks * 4);
            self.mem.w32(c + 0x44, 0);
            self.mem.w32(c + 0x58, 0);
            out = slot as i64;
        }
        self.mem.w32(GLOBALS, 0);
        self.mem.w8(GLOBALS + 0xb, 0xff);
        out
    }

    /// `FUN_002bf038`(node, variable): a looping section's node (the track's +0xc) counts its loops down (+5) and
    /// uses that count as the variable, forgetting the node at −1; then the node's first branch (+0xc, four bytes
    /// each: low, high, the target) whose range holds the variable; 0xffff none
    pub fn branch(&mut self, node: i16, var: u64) -> u16 {
        let mut var = var & 0xff;
        let n = self.node(node as i32);
        let t = self.track(self.mem.r8(n + 2) as u32);
        if self.mem.s16(t + 0xc) as i64 == node as i64 {
            var = (self.mem.r8(t + 5) & 0x7f) as u64;
            if self.mem.r8(n + 9) != 0 {
                let c = self.mem.r8(t + 5).wrapping_sub(1);
                self.mem.w8(t + 5, c);
                if c as i8 == -1 {
                    self.mem.w16(t + 0xc, 0xffff);
                }
            }
        }
        let mut i: i16 = 0;
        loop {
            if self.mem.r8(n + 0xb) as i32 <= i as i32 {
                return 0xffff;
            }
            let e = n + 0xc + 4 * i as u32;
            let (lo, hi) = (self.mem.r8(e) as i8 as i64, self.mem.r8(e + 1) as i8 as i64);
            if lo <= var as i64 && var as i64 <= hi {
                return self.mem.r16(e + 2);
            }
            i = i.wrapping_add(1);
        }
    }

    /// `FUN_002bf290`(router node, value): through the router of the node (its +10: the pairs from the router
    /// table's entry before it to this one's), each pair (high half the value, low half what it becomes) remapping it
    pub fn route(&self, router: i64, mut v: u32) -> u32 {
        if router < 0 {
            return v;
        }
        let b = self.mem.r8(self.node(router as i32) + 10) as u32;
        if b == 0 {
            return v;
        }
        let p = b * 4 + self.mem.r32(self.cur() + 0x28);
        let start = self.mem.r32(p - 4);
        let n = self.mem.r32(p).wrapping_sub(start) as i32;
        let mut at = self.graph().wrapping_add(start.wrapping_mul(4));
        for _ in 0..n.max(0) {
            let w = self.mem.r32(at);
            if v == ((w as i32) >> 16) as u32 {
                v = w & 0xffff;
            }
            at += 4;
        }
        v
    }

    /// `FUN_002bf108`(router node, node, variable): from the node (through the router) past every node with no audio
    /// (its first short below 1): a marker (0) is the track's (+0x10) and is told to the callback; a loop start (−1
    /// with a count at +9) starts the track's count (+5, +0xc) unless already on it; each the branch by the variable
    /// (through the router); the node reached, through the router once more
    pub fn walk(&mut self, router: u16, node: u16, var: u8) -> u16 {
        let mut s = self.route(router as i16 as i64, node as i16 as i32 as u32) as i16;
        let mut n = self.node(s as i32);
        let mut first = self.mem.s16(n);
        let mut t = self.track(self.mem.r8(n + 2) as u32);
        while first < 1 {
            if first == 0 {
                self.mem.w32(t + 0x10, n);
                let song = self.song_index();
                if let Some(cb) = self.marker.as_mut() {
                    cb(song, s);
                }
            } else if first == -1 && self.mem.r8(n + 9) != 0 && s as i32 != self.mem.s16(t + 0xc) as i32 {
                let c = self.mem.r8(n + 9);
                self.mem.w8(t + 5, c);
                self.mem.w16(t + 0xc, s as u16);
            }
            let v = self.branch(s, var as u64);
            s = self.route(router as i16 as i64, v as i16 as i32 as u32) as i16;
            n = self.node(s as i32);
            first = self.mem.s16(n);
            t = self.track(self.mem.r8(n + 2) as u32);
        }
        self.route(router as i16 as i64, s as i32 as u32) as u16
    }

    /// `FUN_002bf320`(event, track): the event grid's cell (+0x20: track · columns · rows + event · columns, plus the
    /// column of the track's current node's +3, or of the first track with one) and its entry (+0x24); the entry's
    /// flag 4 set when its target node's +3 has the top bit; the cell
    pub fn event_entry(&mut self, event: i16, track: i16, out: &mut u32) -> i16 {
        let c = self.cur();
        let t = self.mem.r32(c + 0x174 + (track as i32 * 4) as u32);
        let mut n5 = 0u32;
        if t != 0 && 0 <= self.mem.s16(t + 0x16) {
            n5 = self.node(self.mem.s16(t + 0x16) as i32);
        }
        let g = self.graph();
        let cols = self.mem.r8(g + 0xe) as i32;
        let mut cell = track as i32 * cols * self.mem.r8(g + 0xf) as i32 + event as i32 * cols;
        if n5 == 0 {
            for k in 0..24 {
                let tk = self.mem.r32(c + 0x174 + 4 * k);
                if tk != 0 && 0 <= self.mem.s16(tk + 0x16) {
                    n5 = self.node(self.mem.s16(tk + 0x16) as i32);
                }
                if n5 != 0 {
                    break;
                }
            }
        }
        if n5 != 0 {
            cell += (self.mem.r8(n5 + 3) & 0x7f) as i32;
        }
        let idx = self.mem.r8(self.mem.r32(c + 0x20).wrapping_add(cell as u32)) as u32;
        *out = self.mem.r32(idx * 4 + self.mem.r32(c + 0x24));
        let target = (*out >> 16) as i16;
        if 0 <= target && self.mem.r8(self.node(target as i32) + 3) & 0x80 != 0 {
            *out |= 0x400;
        }
        cell as i16
    }

    /// `FUN_002bf490`(event, the track record, out): the event's entry; kept in `out` when it moves (0x300) or is not a
    /// stop (the low byte not negative); its target (0xffff for a cell below 0)
    pub fn event_target(&mut self, event: i16, track_rec: u32, out: &mut u32) -> u16 {
        let mut e = 0u32;
        let cell = self.event_entry(event, self.mem.r8(track_rec) as i8 as i16, &mut e);
        if cell < 0 {
            e = (e & 0xffff) | 0xffff_0000;
        } else if e & 0x300 != 0 || 0 <= e as u8 as i8 {
            *out = e;
        }
        (e >> 16) as u16
    }
}

/// The stream library under the engine
pub trait PfEnv {
    /// `PathX_TimeMs` 0x223158: the clock in ms
    fn time_ms(&mut self) -> u32;
    /// `PathX_GetPositionMs` 0x2235a0(slot): how far its stream has played
    fn position_ms(&mut self, slot: u32) -> i32;
    /// `StreamSlot_Play` 0x223950(slot, start (100 now, −1 at a time), the track's name, the segment's offset,
    /// volume, beat, immediate): the handle (below 0 failed)
    fn play(&mut self, slot: u32, start: i64, name: u32, offset: u32, volume: u8, beat: u8, immediate: u8) -> i32;
    /// `StreamSlot_Stop` 0x223c40, `StreamSlot_Fade` 0x223ac0
    fn stop(&mut self, slot: u32);
    fn fade(&mut self, slot: u32);
    /// `FUN_00223b70`(slot, volume, ms): the volume (empty in this build: it returns 0)
    fn set_volume(&mut self, slot: u32, volume: i32, ms: Option<i32>);
    /// `StreamSlot_IsDone` 0x2234e0, `StreamSlot_GetPosition` 0x223780 (played, total), `FUN_00223a90`(slot, 0)
    fn is_done(&mut self, slot: u32) -> bool;
    fn get_position(&mut self, slot: u32) -> (i32, i32);
    fn release(&mut self, slot: u32);
}

thread_local! {
    static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// A track record's size as the scheduler saves and restores it
pub const TRACK_RECORD: u32 = 0xd8;

impl Pf<'_> {
    fn save(&self, t: u32) -> Vec<u8> {
        (0..TRACK_RECORD).map(|k| self.mem.r8(t + k)).collect()
    }
    fn restore(&mut self, t: u32, v: &[u8]) {
        for (k, b) in v.iter().enumerate() {
            self.mem.w8(t + k as u32, *b);
        }
    }
    fn c8(&self, a: u32) -> i32 {
        self.mem.r8(a) as i8 as i32
    }
    fn i32_at(&self, a: u32) -> i32 {
        self.mem.r32(a) as i32
    }
    /// A node's segment entry (the table at +0x30, 8 bytes a segment numbered from 1: offset, length)
    fn seg_word(&self, node_rec: u32, word: u32) -> i32 {
        let s = self.mem.s16(node_rec) as i32;
        self.i32_at(self.mem.r32(self.cur() + 0x30).wrapping_add((s * 8) as u32).wrapping_sub(8 - 4 * word))
    }

    /// `PathX_CountActiveTracks` 0x2bee88(songs as bits 24..27): the tracks loaded in them
    pub fn count_active_tracks(&self, mask: u32) -> i32 {
        let mut n = 0;
        for i in 0..4u32 {
            if mask & (0x0100_0000 << i) != 0 && self.mem.r32(SONGS + i * SONG_SIZE + 0x14) != 0 {
                for k in 0..24 {
                    if self.mem.r32(SONGS + i * SONG_SIZE + 0x174 + 4 * k) != 0 {
                        n += 1;
                    }
                }
            }
        }
        n
    }

    /// `FUN_002bef08`(tracks as bits): each playing one at its volume over 500 ms, or faded out (flag 0x40)
    pub fn fade_tracks(&mut self, env: &mut dyn PfEnv, mask: u32) {
        for k in 0..24u32 {
            let t = self.track(k);
            if t != 0 && (mask as i32 >> k) & 1 != 0 && 0 <= self.mem.s16(t + 0x16) {
                if self.mem.r8(t + 0x15) & 0x40 == 0 {
                    env.set_volume(self.mem.r32(t + 0x2c), self.c8(t + 0x14) * self.c8(t + 4) / 100, Some(500));
                } else {
                    env.fade(self.mem.r32(t + 0x2c));
                }
            }
        }
    }

    /// `FUN_002befd8`(node): the segment's offset in the song's audio (0 for none or a node without audio)
    pub fn segment_offset(&self, node: i64) -> u32 {
        if node < 0 || node >= self.mem.r16(self.graph() + 0x12) as i64 {
            return 0;
        }
        let n = self.node(node as i32);
        if 0 < self.mem.s16(n) {
            self.seg_word(n, 0) as u32
        } else {
            0
        }
    }

    /// `FUN_002bfa78`(node): every track waiting on this node (+0xe, with a marker): its volume from the saved entry
    /// (+0x18); with the entry's 2 stopped, with its 1 walked on to its target (+0x1a) and started; no longer waiting
    pub fn release_waiting(&mut self, env: &mut dyn PfEnv, node: i16) {
        for k in 0..24u32 {
            let t = self.track(k);
            if t != 0 && self.mem.r32(t + 0x10) != 0 && 0 <= node && self.mem.s16(t + 0xe) == node {
                let v = (self.mem.r8(t + 0x18) & 0x7f) as i32;
                env.set_volume(self.mem.r32(t + 0x2c), v * self.c8(t + 4) / 100, None);
                self.mem.w8(t + 0x14, v as u8);
                let f = self.mem.r8(t + 0x19);
                if f & 2 == 0 {
                    if f & 1 != 0 {
                        let (to, var) = (self.mem.r16(t + 0x1a), self.mem.r8(t + 1));
                        let n = self.walk(0, to, var);
                        self.mem.w8(t + 6, 1);
                        self.mem.w32(t + 0x24, 0);
                        self.mem.w16(t + 0x16, n);
                        self.mem.w8(t + 0x15, f);
                        self.start_segment(env, t, 0);
                    }
                } else {
                    self.mem.w32(t + 0x30, u32::MAX);
                    self.mem.w16(t + 0x16, 0xffff);
                    self.mem.w32(t + 0x24, 0);
                    self.mem.w32(t + 0x10, 0);
                }
                self.mem.w16(t + 0xe, 0xffff);
            }
        }
    }

    /// `FUN_002bfe28`(track record, immediate): the track's node (+0x16) started on its stream: none stops the
    /// track (and releases those waiting on its event); a short-latency track (+10 under 50 ms) counts its beats
    /// (+6) and times the next (+0x20 the beat, +0x24 when, rounded to 10 ms); a longer one starts at a time (+0x28)
    /// when its stream is idle; the volume (the entry's · the track's, or 255 for an event at a playing stream); the
    /// handle (+0x30) kept in the track's ring (+0xd4: node, beat, handle) when it played
    pub fn start_segment(&mut self, env: &mut dyn PfEnv, t: u32, immediate: u8) -> i32 {
        // (a stopped track releasing a waiter whose walk ends nowhere starts it again, for ever: the game runs out
        // of stack)
        DEPTH.with(|d| d.set(d.get() + 1));
        let deep = DEPTH.with(|d| d.get()) > 64;
        if deep {
            DEPTH.with(|d| d.set(0));
            panic!("pathfinder: endless recursion (the game overflows its stack)");
        }
        let r = self.start_segment_inner(env, t, immediate);
        DEPTH.with(|d| d.set(d.get() - 1));
        r
    }

    fn start_segment_inner(&mut self, env: &mut dyn PfEnv, t: u32, immediate: u8) -> i32 {
        let mut start: i64 = 100;
        let now = env.time_ms();
        if self.mem.s16(t + 0x16) < 0 {
            self.mem.w32(t + 0x30, u32::MAX);
            if self.mem.r32(t + 0x10) != 0 {
                let n = self.mem.s16(t + 0xe);
                self.release_waiting(env, n);
            }
            return 0;
        }
        let pos = env.position_ms(self.mem.r32(t + 0x2c));
        let lat = self.mem.r16(t + 10) as u32;
        if lat < 0x32 {
            if self.mem.r8(t + 6) == 0 {
                self.mem.w8(t + 6, 1);
            }
        } else if pos < 1 {
            self.mem.w32(t + 0x28, now.wrapping_add(lat));
            if 0 < self.i32_at(t + 0x24) {
                let v = self.mem.r32(t + 0x24);
                self.mem.w32(t + 0x28, v);
            }
            start = -1;
            self.mem.w8(t + 6, 0);
        } else {
            self.mem.w8(t + 6, 0);
        }
        let c1 = self.c8(t + 6);
        let vol = if self.mem.r32(self.cur() + 0x44) == 0 || pos == 0 { (self.c8(t + 0x14) * self.c8(t + 4) / 100) as u8 } else { 0xff };
        let node = self.mem.s16(t + 0x16) as i64;
        let off = self.segment_offset(node);
        let beat = self.mem.r8(t + 6);
        let h = env.play(self.mem.r32(t + 0x2c), start, t + 0x34, off, vol, beat, immediate);
        self.mem.w32(t + 0x30, h as u32);
        if lat < 0x32 {
            let n = self.node(self.mem.s16(t + 0x16) as i32);
            let mut x = self.seg_word(n, 1);
            let b = self.mem.r8(n + 5) as i32;
            if b != 0 {
                x /= b;
            }
            self.mem.w32(t + 0x20, x as u32);
            let c2 = self.c8(t + 6);
            self.mem.w32(t + 0x28, 0);
            self.mem.w8(t + 6, (c2 + 1) as u8);
            self.mem.w32(t + 0x24, ((now as i32).wrapping_add(x).wrapping_add(pos) / 10 * 10) as u32);
            if !(c2 < b) {
                self.mem.w8(t + 6, 0);
            }
        } else {
            self.mem.w32(t + 0x20, 0);
        }
        let h = self.i32_at(t + 0x30);
        if 0 <= h {
            let q = self.mem.r32(t + 0xd4) + self.mem.r8(t + 9) as u32 * 0xc;
            self.mem.w32(q + 8, h as u32);
            self.mem.w32(q, self.mem.s16(t + 0x16) as i32 as u32);
            self.mem.w32(q + 4, c1 as u32);
            let p = self.mem.r8(t + 9).wrapping_add(1);
            self.mem.w8(t + 9, p);
            if self.mem.r8(t + 8) <= p {
                self.mem.w8(t + 9, 0);
            }
        }
        h
    }

    /// `FUN_002bfbb8`(track): the next node of a track at its segment's end: within a looping segment's beats
    /// (+6 up to the node's +5) the same node, else the branch by its variable walked on; started; the track
    /// restored when the start fails
    pub fn advance(&mut self, env: &mut dyn PfEnv, k: u32) {
        let t = self.track(k);
        let save = self.save(t);
        let mut s = self.mem.s16(t + 0x16);
        let mut c = self.c8(t + 6);
        let keep = 0 < c && c <= self.mem.r8(self.node(s as i32) + 5) as i32;
        if !keep {
            let s1 = self.mem.s16(t + 0x16);
            c = 1;
            s = -1;
            if 0 <= s1 {
                let var = self.mem.r8(t + 1) as u64;
                s = self.branch(s1, var) as i16;
            }
        }
        if 0 <= s {
            let (from, var) = (self.mem.r16(t + 0x16), self.mem.r8(t + 1));
            s = self.walk(from, s as u16, var) as i16;
        }
        self.mem.w8(t + 6, c as u8);
        self.mem.w32(t + 0x24, 0);
        self.mem.w16(t + 0x16, s as u16);
        if self.start_segment(env, t, 0) < 0 {
            self.restore(t, &save);
        }
    }

    /// `FUN_002c00d0`(the target node's record, the marker's, out): when the lead track (+0) has a marker, how many
    /// ms until the target should start to stay on the beat (out) and the beat count to start on: a 0x7f marker the
    /// lead's position less the target segment's length (wrapped up by the lead's segment until past the target's
    /// latency); a −1 marker the next of the target's beats; else the next bar of the marker's beats (its +7, +8)
    /// at the lead's tempo (soft-float doubles, as the game's libgcc)
    pub fn sync_offset(&mut self, env: &mut dyn PfEnv, n: u32, marker: u32, out: u32) -> i32 {
        self.mem.w32(out, 0);
        let c = self.cur();
        let lead_k = self.i32_at(c);
        if lead_k < 0 {
            return 0;
        }
        let lead = self.track(lead_k as u32);
        if self.mem.r32(lead + 0x10) == 0 || self.mem.r8(marker + 6) == 0 {
            return 0;
        }
        let lrec = self.node(self.mem.s16(lead + 0x16) as i32);
        let cc = self.seg_word(lrec, 1);
        let (l4, l5) = (self.mem.r8(lrec + 4) as i32, self.mem.r8(lrec + 5) as i32);
        let v3 = if l5 != 0 { cc / (l5 * l4) } else { cc };
        let v4 = cc / l4;
        let bc = self.seg_word(n, 1);
        let (n4, n5) = (self.mem.r8(n + 4) as i32, self.mem.r8(n + 5) as i32);
        let v8 = if n5 != 0 { bc / (n5 * n4) } else { bc };
        let (mut b0, mut ac) = env.get_position(self.mem.r32(lead + 0x2c));
        if b0 < 0 || ac < 0 {
            return 0;
        }
        if self.i32_at(lead + 0x20) != 0 {
            if self.mem.r8(lead + 6) != 0 {
                ac = (l5 + 1 - self.c8(lead + 6)) * self.i32_at(lead + 0x20) + ac;
            }
            b0 = cc - ac;
        }
        // (the game traps here on a lead segment of no length)
        assert!(v3 != 0, "pathfinder: the game traps (a lead segment of no length)");
        let t1 = self.track(self.mem.r8(n + 2) as u32);
        let lat = self.mem.r16(t1 + 10) as i32;
        let v9 = b0 + lat;
        let kind = self.mem.r8(marker + 6);
        if kind == 0x7f {
            let mut o = ac - bc;
            self.mem.w32(out, o as u32);
            if lat <= o {
                return 1;
            }
            // (a lead segment of no length never gets there: the game loops for ever)
            assert!(0 < cc, "pathfinder: the game loops for ever (a lead segment of no length)");
            loop {
                o = o.wrapping_add(cc);
                self.mem.w32(out, o as u32);
                if lat <= o {
                    return 1;
                }
            }
        }
        if kind == 0xff {
            let q = (v9 % bc) / v8;
            self.mem.w32(out, (q * v8 + v8 - v9 % bc) as u32);
            return q + 2;
        }
        if l5 == 0 || self.mem.r8(marker + 8) == 0 || self.mem.r8(marker + 7) == 0 {
            return 1;
        }
        let beats = self.mem.r8(lrec + 6) as i8;
        let mut d2 = beats as f64;
        if beats < 0 {
            d2 += 256.0;
        }
        let d = v3 as f64 * (d2 / kind as f64);
        let i3 = d as i32;
        let r = v9 % v4;
        let i4 = (self.mem.r8(marker + 8) as i32 - 1) * i3;
        let mut o = -r;
        if 0 < i4 {
            o += i4;
        }
        let step = self.mem.r8(marker + 7) as i32 * i3;
        // (a bar of no length never gets there: the game loops for ever)
        assert!(0 <= o || 0 < step, "pathfinder: the game loops for ever (a bar of no length)");
        while o < 0 {
            o = o.wrapping_add(step);
        }
        o += v9 - r;
        self.mem.w32(out, o as u32);
        1
    }

    /// `FUN_002bf508`(track): the next node at a segment's end. A pending walk (the song's +0x34 below 0: its
    /// complement the node) is walked to and started. Else the song's event (+0x34) for this track: its entry
    /// (volume, waiting on a marker, the flag 0x40); an entry that moves (0x300) walks to its target, synced to the
    /// lead track unless this is the lead, starting now or at its time (+0x24, the song's latest at +0x40); when it
    /// cannot move the track stops (or, not immediate and before the song's latest, waits: the track restored). The
    /// track's bit in the song's +0x44; the node started, or 0xffff
    pub fn next_segment(&mut self, env: &mut dyn PfEnv, k: i32) -> u16 {
        let c = self.cur();
        let mut beat = 1u8;
        let mut started: i64 = 0;
        let ev = self.mem.r32(c + 0x34) as i16;
        let t = self.track(k as u32);
        let save = self.save(t);
        let saved = self.i32_at(t + 0x14);
        let mut e = (saved as u32 & 0xffff_0000) | (saved as u32 & 0xff);
        let immediate = self.mem.r8(c + 0x37);
        'done: {
            if ev < 0 {
                let target = !ev;
                let rec = self.node(target as i32);
                let (from, var) = (self.mem.r16(t + 0x16), self.mem.r8(t + 1));
                let n = self.walk(from, target as u16, var);
                self.mem.w16(t + 0x16, n);
                started = self.start_segment(env, t, immediate) as i64;
                if self.mem.s16(rec) == 0 {
                    self.mem.w32(t + 0x10, rec);
                }
                break 'done;
            }
            let target = self.event_target(ev, t, &mut e) as i16;
            if 0 <= e as u8 as i8 {
                if e & 0x400 == 0 {
                    self.mem.w8(t + 0x14, e as u8);
                    self.mem.w16(t + 0xe, ev as u16);
                    self.mem.w32(t + 0x18, saved as u32);
                } else {
                    self.mem.w8(t + 0x19, 0);
                    self.mem.w16(t + 0xe, 0xffff);
                    self.mem.w16(t + 0x1a, 0xffff);
                }
            }
            if e & 0x4000 != 0 {
                let f = self.mem.r8(t + 0x15) | 0x40;
                self.mem.w8(t + 0x15, f);
            }
            if e & 0x300 == 0 {
                started = -1;
                break 'done;
            }
            self.mem.w32(t + 0x24, 0);
            self.mem.w8(t + 7, 0);
            if 0 <= target {
                if e & 0x400 == 0 {
                    let w = if self.mem.r32(t + 0x10) != 0 { 1 } else { 2 };
                    self.mem.w8(t + 0x19, w);
                    self.mem.w16(t + 0xe, ev as u16);
                    self.mem.w16(t + 0x1a, target as u16);
                } else {
                    self.mem.w8(t + 0x19, 0);
                    self.mem.w16(t + 0xe, 0xffff);
                    self.mem.w16(t + 0x1a, 0xffff);
                }
                let (from, var) = (self.mem.r16(t + 0x16), self.mem.r8(t + 1));
                let n = self.walk(from, target as u16, var) as i16;
                if 0 <= n {
                    if k != self.i32_at(c) {
                        let rec = self.node(n as i32);
                        let marker = self.mem.r32(t + 0x10);
                        beat = self.sync_offset(env, rec, marker, t + 0x24) as u8;
                    }
                    let mut at = self.i32_at(t + 0x24);
                    if 0 < at {
                        let now = env.time_ms();
                        let v = self.mem.r32(t + 0x24).wrapping_add(now);
                        self.mem.w32(t + 0x24, v);
                        if self.mem.r32(c + 0x40) < v {
                            self.mem.w32(c + 0x40, v);
                        }
                        at = self.i32_at(t + 0x24);
                    }
                    self.mem.w16(t + 0x16, n as u16);
                    self.mem.w8(t + 6, beat);
                    if at == 0 {
                        started = self.start_segment(env, t, immediate) as i64;
                    }
                    break 'done;
                }
            }
            if self.mem.r8(t + 0x15) & 0x40 != 0 {
                return 0xffff;
            }
            if 0 <= saved {
                if immediate == 0 {
                    if self.mem.r32(c + 0x3c) < self.mem.r32(c + 0x40) {
                        let v = self.mem.r32(c + 0x40);
                        self.mem.w32(c + 0x3c, v);
                        self.restore(t, &save);
                        return 0xffff;
                    }
                } else {
                    env.stop(self.mem.r32(t + 0x2c));
                }
            }
            self.mem.w32(t + 0x10, 0);
            self.mem.w16(t + 0x16, 0xffff);
            self.mem.w32(t + 0x30, u32::MAX);
        }
        let m = self.mem.r32(c + 0x44) | 1 << (self.mem.r8(t) & 31);
        self.mem.w32(c + 0x44, m);
        if 0 <= started {
            return self.mem.r16(t + 0x16);
        }
        if (e as u8 as i8) < 0 {
            self.restore(t, &save);
        }
        0xffff
    }
}

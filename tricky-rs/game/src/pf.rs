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
    /// `StreamSlot_IsDone` 0x2234e0, `StreamSlot_GetPosition` 0x223780 (its answer: the handle playing; played,
    /// total), `FUN_00223a90`(slot, 0)
    fn is_done(&mut self, slot: u32) -> bool;
    fn get_position(&mut self, slot: u32) -> (i32, i32, i32);
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
        let (_, mut b0, mut ac) = env.get_position(self.mem.r32(lead + 0x2c));
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

/// The stream driver's calls the controls make
pub trait PfDriver: PfEnv {
    /// `PathX_FreeTrack` 0x223450, `PathX_StopDriver` 0x223710
    fn free_track(&mut self, slot: u32);
    fn stop_driver(&mut self);
    /// `StreamSlot_SetPitch` 0x223b78(slot, pitch)
    fn set_pitch(&mut self, slot: u32, pitch: u8);
    /// `StreamSlot_Fade` 0x223ac0(slot, volume, ms)
    fn fade_to(&mut self, slot: u32, volume: i32, ms: u64);
    /// `FUN_002231a8`: the driver's own bytes a track needs
    fn track_overhead(&mut self) -> i32;
    /// `PathX_Init` 0x223648(the update callbacks 0x2c0488 / 0x2c0898, the stop 0x2bedb8)
    fn init_driver(&mut self);
    /// `PathX_OpenSong` 0x2231d8(name, a, ring size, b, buffer, its size): the stream slot (below 0 failed)
    fn open_song(&mut self, name: u32, a: u32, n: i32, b: u32, buffer: u32, size: i32) -> i32;
}

impl Pf<'_> {
    fn started(&self) -> bool {
        self.mem.r8(GLOBALS + 8) != 0
    }
    fn unselect(&mut self) {
        self.mem.w32(GLOBALS, 0);
        self.mem.w8(GLOBALS + 0xb, 0xff);
    }
    fn busy(&self) -> bool {
        !self.started() || 0 <= self.song_index()
    }
    /// The songs a mask names (bits 24..27) that are loaded, each selected in turn
    fn songs_in(&mut self, mask: u32, mut f: impl FnMut(&mut Self)) {
        for i in 0..4u32 {
            if mask & (0x0100_0000 << i) != 0 && 0 <= self.select(i as u64) {
                f(self);
            }
        }
    }

    /// `FUN_002c0488` (the driver's update): for each song, the lead track (the longest latency among those at a
    /// node with a marker); then each unpaused track: a pending event not yet taken by it, due within the look-ahead
    /// (+8), takes it (`next_segment`); a playing track whose stream is done: (taking an event) the song's earliest
    /// start (+0x3c, +0x38 the track that set it) from its position; when its time is near (the look-ahead, or +0xc
    /// for a beat-counted track) or its position past its latency, the next segment (`advance`, or the event's);
    /// a track waiting to start (+0x28) is released at its time. All tracks done with the event (+0x44 all set) and
    /// past its latest start: the volumes eased in and the event queue moves up one
    pub fn update(&mut self, env: &mut dyn PfEnv) {
        if !self.started() || self.mem.r8(GLOBALS + 9) != 0 || 0 <= self.song_index() {
            return;
        }
        let mut now = 0u32;
        for i in 0..4u64 {
            if self.select(i) < 0 {
                continue;
            }
            let c = self.cur();
            self.lead_track();
            for k in 0..24u32 {
                let t = self.track(k);
                if t == 0 || self.mem.r8(t + 2) != 0 {
                    continue;
                }
                now = env.time_ms();
                let lat = self.mem.r16(t + 10) as u32;
                let due = now.wrapping_add(lat) as i32;
                let mut go = 0 <= self.mem.s16(t + 0x16) && self.mem.r32(t + 0x10) != 0;
                let mask = self.mem.r32(c + 0x44);
                let taking = mask != 0 && (mask as i32 >> k) & 1 == 0;
                if taking && (self.mem.r8(c + 0x37) != 0 || self.mem.r32(c + 0x3c) != 0) {
                    let at = self.mem.r32(c + 0x3c);
                    let d = if at == 0 { 0 } else { at.wrapping_sub(due as u32) as i32 };
                    if d <= self.i32_at(c + 8) {
                        go = true;
                        if (self.next_segment(env, k as i32) as i16) < 0 {
                            continue;
                        }
                    }
                }
                if !go {
                    continue;
                }
                if self.i32_at(t + 0x28) == 0 {
                    if !env.is_done(self.mem.r32(t + 0x2c)) {
                        continue;
                    }
                    let pos = env.position_ms(self.mem.r32(t + 0x2c));
                    if taking && self.mem.r8(c + 0x37) == 0 {
                        let mut p = pos;
                        if self.i32_at(t + 0x20) != 0 && self.mem.r8(t + 6) != 0 {
                            let n = self.node(self.mem.s16(t + 0x16) as i32);
                            p = (self.mem.r8(n + 5) as i32 + 1 - self.c8(t + 6)) * self.i32_at(t + 0x20) + pos;
                        }
                        let v = now.wrapping_add(p as u32);
                        let earliest = self.mem.r32(c + 0x3c);
                        let set = self.mem.r32(c + 0x38) == k
                            || v < earliest
                            || (earliest == 0 && self.mem.r32(c) == k);
                        if set {
                            self.mem.w32(c + 0x3c, v);
                            self.mem.w32(c + 0x38, k);
                        }
                    }
                    let at = self.i32_at(t + 0x24);
                    let wait = if at < 1 {
                        (lat as i32) < pos
                    } else {
                        let ahead = if lat < 0x32 { self.mem.r32(c + 0xc) } else { self.mem.r32(c + 8) };
                        due.wrapping_add(ahead as i32) < at
                    };
                    if !wait
                        && (!taking
                            || (self.mem.r8(t + 6) != 0 && self.i32_at(t + 0x20) != 0)
                            || (self.next_segment(env, k as i32) as i16) < 0)
                    {
                        self.advance(env, k);
                    }
                } else if self.i32_at(t + 0x28) <= now as i32 {
                    env.release(self.mem.r32(t + 0x2c));
                    self.mem.w32(t + 0x28, 0);
                }
            }
            let c = self.cur();
            if self.mem.r32(c + 0x44) == u32::MAX && self.mem.r32(c + 0x40) < now {
                self.fade_tracks(env, 0xff_ffff);
                for j in 0..15u32 {
                    for w in 0..5u32 {
                        let v = self.mem.r32(c + 0x34 + 0x14 * (j + 1) + 4 * w);
                        self.mem.w32(c + 0x34 + 0x14 * j + 4 * w, v);
                    }
                }
                self.mem.w32(c + 0x170, 0);
            }
        }
        self.unselect();
    }

    /// Both callbacks first find the lead track: the longest latency (+10) among the unpaused tracks at a node
    /// with a marker (+0: its number, +4: the latency)
    fn lead_track(&mut self) {
        let c = self.cur();
        self.mem.w32(c + 4, 0);
        self.mem.w32(c, u32::MAX);
        for k in 0..24u32 {
            let t = self.track(k);
            if t != 0 && self.mem.r8(t + 2) == 0 && 0 <= self.mem.s16(t + 0x16) && self.mem.r32(t + 0x10) != 0 {
                let lat = self.mem.r16(t + 10) as i32;
                if self.i32_at(c + 4) < lat {
                    self.mem.w32(c + 4, lat as u32);
                    self.mem.w32(c, k);
                }
            }
        }
    }

    /// `FUN_002c0898` (the driver's beat callback): the lead track; each unpaused beat-counted track (latency under
    /// 50) not about to take a pending event advances once its start time (+0x24) has passed (or, with none, its
    /// position its latency)
    pub fn beat(&mut self, env: &mut dyn PfEnv) {
        if !self.started() || self.mem.r8(GLOBALS + 9) != 0 || 0 <= self.song_index() {
            return;
        }
        for i in 0..4u64 {
            if self.select(i) < 0 {
                continue;
            }
            let c = self.cur();
            self.lead_track();
            for k in 0..24u32 {
                let t = self.mem.r32(self.cur() + 0x174 + 4 * k);
                if t == 0 || self.mem.r8(t + 2) != 0 || 0x32 <= self.mem.r16(t + 10) {
                    continue;
                }
                let now = env.time_ms();
                let lat = self.mem.r16(t + 10) as u32;
                let due = now.wrapping_add(lat) as i32;
                let playing = 0 <= self.mem.s16(t + 0x16) && self.mem.r32(t + 0x10) != 0;
                let mask = self.mem.r32(self.cur() + 0x44);
                let taking = mask != 0 && (mask >> k) & 1 == 0;
                if taking && (self.mem.r8(c + 0x37) != 0 || self.mem.r32(c + 0x3c) != 0) {
                    let at = self.i32_at(c + 0x3c);
                    let d = if at == 0 { 0 } else { at.wrapping_sub(due) };
                    if d <= self.i32_at(self.cur() + 8) {
                        continue;
                    }
                }
                if playing {
                    let pos = env.position_ms(self.mem.r32(t + 0x2c)) as i64;
                    let mut wait = due < self.i32_at(t + 0x24);
                    if self.i32_at(t + 0x24) < 1 {
                        wait = (lat as i64) < pos;
                    }
                    if !wait {
                        self.advance(env, k);
                    }
                }
            }
        }
        self.unselect();
    }

    /// `FUN_002c1018`(ring size): the bytes a track record needs (the driver's, 0xd8, the ring), to 16 past
    pub fn track_bytes(env: &mut dyn PfDriver, n: i32) -> i32 {
        let v = env.track_overhead() + 0xd8 + n * 0xc;
        (v / 16) * 16 + 0x10
    }

    /// `FUN_002bedb8` (the driver's stop callback): every track freed (`PathX_StopTracks`(−1)), none selected, not
    /// started
    pub fn shut_down(&mut self, env: &mut dyn PfDriver) {
        self.stop_tracks(env, u32::MAX);
        self.mem.w8(GLOBALS + 0xb, 0xff);
        self.mem.w8(GLOBALS + 8, 0);
    }

    /// `FUN_002c1068`(songs, look-ahead, beat look-ahead): +8 and +0xc of each
    pub fn set_lookahead(&mut self, mask: u32, a: u32, b: u32) {
        if !self.started() || 0 <= self.song_index() {
            return;
        }
        self.songs_in(mask, |p| {
            let c = p.cur();
            p.mem.w32(c + 0xc, b);
            p.mem.w32(c + 8, a);
        });
        self.unselect();
    }

    /// `FUN_002bedf0`(id: one song bit in 24..27, one track bit below): that track's record, or 0
    pub fn track_of(&self, id: u32) -> u32 {
        for i in 0..4u32 {
            if 0x0100_0000 << i == id & 0x0f00_0000 && self.mem.r32(SONGS + i * SONG_SIZE + 0x14) != 0 {
                for k in 0..24u32 {
                    if 1 << k == id & 0xff_ffff {
                        return self.mem.r32(SONGS + i * SONG_SIZE + 0x174 + 4 * k);
                    }
                }
            }
        }
        0
    }

    /// `FUN_002c1130`(id, latency): the track's latency (+10); −8 for no track
    pub fn set_latency(&mut self, id: u32, lat: u16) -> i32 {
        let t = self.track_of(id);
        if t == 0 {
            return -8;
        }
        self.mem.w16(t + 10, lat);
        0
    }

    /// `FUN_002c1168`(id) (`PathX_GetSongState`): the track's stream slot (+0x2c); −8 for no track
    pub fn slot_of(&self, id: u32) -> i32 {
        let t = self.track_of(id);
        if t == 0 {
            return -8;
        }
        self.i32_at(t + 0x2c)
    }

    /// `FUN_002c1190`(id, out: five words): what the track is playing: the node and beat of the stream's handle in
    /// its ring (−1 when not found, the ring's entry dropped when the stream reports none), the position (played +
    /// total, total), the current node; the stream's answer (−8 for no track)
    pub fn playing(&mut self, env: &mut dyn PfEnv, id: u32, out: u32) -> i32 {
        self.mem.w32(out + 4, 0);
        self.mem.w32(out + 8, 0);
        self.mem.w32(out, u32::MAX);
        let t = self.track_of(id);
        if t == 0 {
            return -8;
        }
        let (h, played, total) = env.get_position(self.mem.r32(t + 0x2c));
        if h < 0 {
            return h;
        }
        self.mem.w32(out + 8, total as u32);
        self.mem.w32(out + 4, played.wrapping_add(total) as u32);
        let node = self.mem.s16(t + 0x16) as i32;
        self.mem.w32(out + 0x10, node as u32);
        let mut q = 0u32;
        while q < self.mem.r8(t + 8) as u32 {
            let e = self.mem.r32(t + 0xd4) + 0xc * q;
            if self.i32_at(e + 8) == h {
                if 0 <= total {
                    let (a, b) = (self.mem.r32(e), self.mem.r32(e + 4));
                    self.mem.w32(out, a);
                    self.mem.w32(out + 0xc, b);
                    return h;
                }
                self.mem.w32(e + 8, u32::MAX);
            }
            q += 1;
        }
        h
    }

    /// `FUN_002c1288`(songs, variable): every track's path variable (+1), for a variable under 128; −0x12 when busy
    pub fn set_variable(&mut self, mask: u32, var: u64) -> i32 {
        if self.busy() {
            return -0x12;
        }
        if var < 0x80 {
            self.songs_in(mask, |p| {
                for k in 0..24u32 {
                    let t = p.track(k);
                    if t != 0 {
                        p.mem.w8(t + 1, var as u8);
                    }
                }
            });
        }
        self.unselect();
        0
    }

    /// `FUN_002c1380`(songs): the event queue's sixteen masks cleared
    pub fn clear_events(&mut self, mask: u32) {
        self.songs_in(mask, |p| {
            let c = p.cur();
            for j in 0..16u32 {
                p.mem.w32(c + 0x44 + 0x14 * j, 0);
            }
        });
        self.unselect();
    }

    /// `FUN_002c1430`(songs): the events queued in the first loaded song named
    pub fn queued_events(&mut self, mask: u32) -> i32 {
        for i in 0..4u32 {
            if mask & (0x0100_0000 << i) != 0 && 0 <= self.select(i as u64) {
                let c = self.cur();
                let n = (0..16u32).filter(|j| self.mem.r32(c + 0x44 + 0x14 * j) != 0).count() as i32;
                self.unselect();
                return n;
            }
        }
        self.unselect();
        0
    }

    /// `FUN_002c1500`(songs, out): the first named song with an event under way: its event (and its earliest start
    /// into `out` when given); 0xffff none
    pub fn current_event(&mut self, mask: u32, out: Option<u32>) -> u16 {
        for i in 0..4u32 {
            if mask & (0x0100_0000 << i) != 0 && 0 <= self.select(i as u64) && self.mem.r32(self.cur() + 0x44) != 0 {
                let c = self.cur();
                if let Some(o) = out {
                    let v = self.mem.r32(c + 0x3c);
                    self.mem.w32(o, v);
                }
                let e = self.mem.r16(c + 0x34);
                self.unselect();
                return e;
            }
        }
        self.unselect();
        0xffff
    }

    /// `FUN_002c15c8`(tracks, event (song in bits 8..11), priority): an event for the named tracks of a song, queued
    /// by priority (behind those of the same or higher); its mask the tracks not named (−1 less each), immediate when
    /// any of their entries is a stop (0x80 in its second byte); priority below 0 releases those waiting on it
    /// instead; priority 2 is immediate at priority 1 and, at the head, stops the song and unpauses
    pub fn post_event(&mut self, env: &mut dyn PfDriver, tracks: u32, event: i16, prio: i8) -> i32 {
        if self.busy() {
            return -0x12;
        }
        let ev = event as i32 as u32 & 0xff;
        let song = ((event as i32 as u32) & 0xf00) >> 8;
        if 0 <= self.select(song as u64) && ev < self.mem.r8(self.graph() + 0xf) as u32 {
            let c = self.cur();
            let g = self.graph();
            let (rows, cols) = (self.mem.r8(g + 0xf) as u32, self.mem.r8(g + 0xe) as u32);
            let mut mask = u32::MAX;
            let mut imm = 0u8;
            for k in 0..24u32 {
                if self.track(k) != 0 && (tracks as i32 >> k) & 1 != 0 {
                    let cell = self.mem.r8(self.mem.r32(c + 0x20) + (k * rows + ev) * cols) as u32;
                    let entry = self.mem.r32(cell * 4 + self.mem.r32(c + 0x24));
                    mask = mask.wrapping_sub(1 << k);
                    if (entry >> 8) as u8 & 0x80 != 0 {
                        imm = 1;
                    }
                }
            }
            if prio < 0 {
                self.release_waiting(env, ev as i16);
            } else {
                let mut j = 0u32;
                while j < 16 && self.mem.r32(c + 0x44 + 0x14 * j) != 0 && prio as i32 <= self.c8(c + 0x36 + 0x14 * j) {
                    j += 1;
                }
                if j < 16 {
                    let mut i = 15u32;
                    while j < i {
                        i -= 1;
                        for w in 0..5u32 {
                            let v = self.mem.r32(c + 0x34 + 0x14 * i + 4 * w);
                            self.mem.w32(c + 0x34 + 0x14 * (i + 1) + 4 * w, v);
                        }
                    }
                    let e = c + 0x34 + 0x14 * j;
                    self.mem.w16(e, ev as u16);
                    self.mem.w8(e + 2, prio as u8);
                    self.mem.w8(e + 3, imm);
                    self.mem.w32(e + 4, 0);
                    self.mem.w32(e + 8, 0);
                    self.mem.w32(e + 0xc, 0);
                    self.mem.w32(e + 0x10, mask);
                    if prio == 2 {
                        self.mem.w8(e + 3, 1);
                        self.mem.w8(e + 2, 1);
                        if j == 0 {
                            self.unselect();
                            self.stop_song(env, u32::MAX);
                            self.set_pitch(env, u32::MAX, 0);
                        }
                    }
                }
            }
        }
        self.unselect();
        0
    }

    /// `FUN_002c1890`(songs and tracks, node, immediate): a jump of the named track to a node at the head of the
    /// queue (−1 − node; the other tracks' bits set; +0x58 cleared), when the node is the track's
    pub fn jump_to(&mut self, mask: u32, node: i64, imm: u8) -> i32 {
        if self.busy() {
            return -0x12;
        }
        'songs: for i in 0..4u32 {
            if mask & (0x0100_0000 << i) == 0 || self.select(i as u64) < 0 {
                continue;
            }
            if node < 0 || (self.mem.r16(self.graph() + 0x12) as i64) < node {
                break;
            }
            for k in 0..24u32 {
                let bit = 1u32 << k;
                if mask & bit != 0 {
                    let n = self.node(node as i32);
                    if n == 0 {
                        break 'songs;
                    }
                    let b = 1u32 << (self.mem.r8(n + 2) & 31);
                    if b != bit {
                        break 'songs;
                    }
                    let c = self.cur();
                    self.mem.w16(c + 0x34, (-1 - node) as u16);
                    self.mem.w8(c + 0x36, 1);
                    self.mem.w8(c + 0x37, imm);
                    self.mem.w32(c + 0x38, b);
                    self.mem.w32(c + 0x3c, 0);
                    self.mem.w32(c + 0x40, 0);
                    self.mem.w32(c + 0x44, !b);
                    self.mem.w32(c + 0x58, 0);
                }
            }
        }
        self.unselect();
        0
    }

    /// `PathX_StopTracks` 0x2c1a38(songs and tracks): the first named track found is freed (a song left with none
    /// loses its graph); the driver stopped once no song has tracks; the last track word read
    pub fn stop_tracks(&mut self, env: &mut dyn PfDriver, mask: u32) -> u32 {
        if self.busy() {
            return 0;
        }
        let mut last = 0u32;
        'songs: for i in 0..4u32 {
            if mask & (0x0100_0000 << i) == 0 || self.select(i as u64) < 0 {
                continue;
            }
            for k in 0..24u32 {
                let at = self.cur() + 0x174 + 4 * k;
                last = self.mem.r32(at);
                if last != 0 && (mask as i32 >> k) & 1 != 0 {
                    let slot = self.mem.r32(last + 0x2c);
                    self.mem.w32(at, 0);
                    env.free_track(slot);
                    if self.count_active_tracks(0x0100_0000 << i) == 0 {
                        let c = self.cur();
                        self.mem.w32(c + 0x14, 0);
                    }
                    break 'songs;
                }
            }
        }
        if self.started() && self.count_active_tracks(0x0f00_0000) == 0 {
            env.stop_driver();
        }
        self.unselect();
        last
    }

    /// `FUN_002c1b98`(songs and tracks, pitch): each named track's pitch (+2: 0 runs, else paused); all songs' also
    /// the pause flag (0x3495d1)
    pub fn set_pitch(&mut self, env: &mut dyn PfDriver, mask: u32, pitch: u8) {
        if !self.started() || 0 <= self.song_index() {
            return;
        }
        if mask == u32::MAX {
            self.mem.w8(GLOBALS + 9, pitch);
        }
        self.songs_in(mask, |p| {
            for k in 0..24u32 {
                let t = p.track(k);
                if t != 0 && (mask as i32 >> k) & 1 != 0 {
                    env.set_pitch(p.mem.r32(t + 0x2c), pitch);
                    p.mem.w8(t + 2, pitch);
                }
            }
        });
        self.unselect();
    }

    /// `PathX_StopSong` 0x2c1cc0(songs): every track's stream stopped and its node cleared
    pub fn stop_song(&mut self, env: &mut dyn PfDriver, mask: u32) {
        if !self.started() || 0 <= self.song_index() {
            return;
        }
        self.songs_in(mask, |p| {
            for k in 0..24u32 {
                let t = p.track(k);
                if t != 0 {
                    env.stop(p.mem.r32(t + 0x2c));
                    p.mem.w16(t + 0x16, 0xffff);
                }
            }
        });
        self.unselect();
    }

    /// `PathX_SetVolume` 0x2c1db8(songs and tracks, volume 0..100): each named track's (+4) and its stream's; all
    /// songs' also the default (0x3495d2)
    pub fn set_volume(&mut self, env: &mut dyn PfDriver, mask: u32, volume: i8) {
        if !self.started() || 0 <= self.song_index() {
            return;
        }
        let v = (volume as i32).clamp(0, 100);
        if mask == u32::MAX {
            self.mem.w8(GLOBALS + 0xa, v as u8);
        }
        self.songs_in(mask, |p| {
            for k in 0..24u32 {
                let t = p.track(k);
                if t != 0 && (mask as i32 >> k) & 1 != 0 {
                    p.mem.w8(t + 4, v as u8);
                    env.set_volume(p.mem.r32(t + 0x2c), p.c8(t + 0x14) * v / 100, None);
                }
            }
        });
        self.unselect();
    }

    /// `PathX_FadeTracks` 0x2c1f10(songs and tracks, volume, ms): each named track's stream faded to the volume times
    /// its own
    pub fn fade_to(&mut self, env: &mut dyn PfDriver, mask: u32, volume: i32, ms: u64) {
        if !self.started() || 0 <= self.song_index() {
            return;
        }
        self.songs_in(mask, |p| {
            for k in 0..24u32 {
                let t = p.track(k);
                if t != 0 && (mask as i32 >> k) & 1 != 0 {
                    env.fade_to(p.mem.r32(t + 0x2c), volume * p.c8(t + 4) / 100, ms);
                }
            }
        });
        self.unselect();
    }

    /// `PathX_LoadSong` 0x2c0cb0(song, track, the file's name at `name`, latency, a, ring size, b, the track record,
    /// its bytes): the track record set up (its number, the song's +0x10, the default volume, the latency, the ring,
    /// cleared state, the name and its base name as "%-8s" with every '.' cut), the driver started with the first
    /// track, the stream opened in the rest of the record's bytes (16-aligned after the ring); the id (song bit
    /// 24 + n, track bit), −0xd for a track in use, −0x12 when busy, or the driver's error
    #[allow(clippy::too_many_arguments)]
    pub fn load_song(&mut self, env: &mut dyn PfDriver, song: u32, track: u32, name_at: u32, name: &[u8], lat: u16, a: u32, n: i32, b: u32, rec: u32, size: i32) -> i32 {
        if !self.started() || 0 <= self.song_index() {
            return -0x12;
        }
        let mut out = -1;
        if 0 <= self.select(song as u64) {
            out = -0xd;
            let c = self.cur();
            if self.mem.r32(c + 0x174 + 4 * track) == 0 {
                let ring = rec + 0xd8;
                self.mem.w32(rec + 0xd4, ring);
                for q in 0..n.max(0) as u32 {
                    self.mem.w32(ring + 0xc * q + 8, u32::MAX);
                    self.mem.w32(ring + 0xc * q, 0);
                    self.mem.w32(ring + 0xc * q + 4, 0);
                }
                self.mem.w8(rec, track as u8);
                self.mem.w8(rec + 2, 0);
                let x10 = self.mem.r8(c + 0x10);
                self.mem.w8(rec + 1, x10);
                self.mem.w16(rec + 10, lat);
                let dv = self.mem.r8(GLOBALS + 0xa);
                self.mem.w8(rec + 4, dv);
                self.mem.w32(rec + 0x10, 0);
                self.mem.w8(rec + 8, n as u8);
                self.mem.w32(rec + 0x14, 0xffff_00ff);
                self.mem.w8(rec + 0x14, 0x7f);
                self.mem.w32(rec + 0x18, 0xffff_00ff);
                self.mem.w32(rec + 0x1c, 0xffff_00ff);
                self.mem.w8(rec + 6, 0);
                self.mem.w8(rec + 7, 0);
                self.mem.w8(rec + 5, 0);
                self.mem.w32(rec + 0x24, 0);
                self.mem.w32(rec + 0x20, 0);
                self.mem.w32(rec + 0x28, 0);
                self.mem.w32(rec + 0x30, 0);
                self.mem.w8(rec + 9, 0);
                self.mem.w16(rec + 0xc, 0xffff);
                self.mem.w16(rec + 0xe, 0xffff);
                // the name (strcpy) and its base name ("%-8s", then every '.' cut)
                let len = name.iter().position(|b| *b == 0).unwrap_or(name.len());
                for (q, ch) in name[..len].iter().enumerate() {
                    self.mem.w8(rec + 0x34 + q as u32, *ch);
                }
                self.mem.w8(rec + 0x34 + len as u32, 0);
                let sep = |ch: u8| ch == 0x5c || ch == b'/' || ch == b':';
                let base: &[u8] = match name[..len].iter().rposition(|ch| sep(*ch)) {
                    Some(p) => &name[p + 1..len],
                    None => &name[..len],
                };
                let mut s: Vec<u8> = base.to_vec();
                while s.len() < 8 {
                    s.push(b' ');
                }
                for (q, ch) in s.iter().enumerate() {
                    self.mem.w8(rec + 0xb4 + q as u32, if *ch == b'.' { 0 } else { *ch });
                }
                self.mem.w8(rec + 0xb4 + s.len() as u32, 0);
                self.mem.w32(c + 0x174 + 4 * track, rec);
                if self.count_active_tracks(0x0f00_0000) == 1 {
                    env.init_driver();
                }
                let off = 0xd8 + n * 0xc;
                let buf = rec.wrapping_add(((off / 16) * 16 + 0x10) as u32);
                let slot = env.open_song(name_at, a, n, b, buf, size - (buf.wrapping_sub(rec)) as i32);
                self.mem.w32(rec + 0x2c, slot as u32);
                if slot < 0 {
                    let c = self.cur();
                    self.mem.w32(c + 0x174 + 4 * track, 0);
                    out = slot;
                } else {
                    self.mem.w32(rec + 0x30, u32::MAX);
                    out = ((0x0100_0000u32 << (song & 31)).wrapping_add(1 << (track & 31))) as i32;
                }
            }
        }
        self.unselect();
        out
    }
}

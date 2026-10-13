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

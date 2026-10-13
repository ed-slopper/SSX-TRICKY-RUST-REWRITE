//! Pathfinder's graph walk (board row G10b, part 1) against the game in the function runner (TRICKY_ELF and
//! TRICKY_MUSIC_DIR, the songs' `.mpf` files out of DATA/AUDIO/MUSIC.BIG; skipped without them): every song graph
//! registered by `PathX_RegisterSong` on both sides, then random branches, routers, walks and event lookups on random
//! track records: the song table, the graph, the tracks and the marker callback's calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::{Mem, Pf, PfDriver, PfEnv, AUTHOR, GLOBALS, SONGS, SONG_SIZE};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n.max(1)
    }
}

const MARKER: u32 = 0x01f0_0000;
const TRACK_SIZE: u32 = 0xd8;

fn read(r: &mut Runner, at: u32, n: u32) -> Vec<u8> {
    let mut v = vec![0u8; n as usize];
    r.mem.read(at, &mut v).unwrap();
    v
}

#[test]
fn graph_walk_is_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let Ok(dir) = std::env::var("TRICKY_MUSIC_DIR") else { return };
    let mut songs: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "mpf"))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .collect();
    songs.sort();
    assert!(!songs.is_empty());
    let calls: Rc<RefCell<Vec<(i8, i16)>>> = Rc::new(RefCell::new(vec![]));
    let c = calls.clone();
    r.stub(MARKER, move |cpu, _| c.borrow_mut().push((cpu.gpr(4) as i8, cpu.gpr(5) as i16)));
    for a in [0x2beca0u32, 0x2bed58] {
        r.allow(a);
    }
    let graph_at = r.mem.alloc(0x40000);
    let tracks_at = r.mem.alloc(TRACK_SIZE * 24);
    let author = r.mem.read_u32(AUTHOR);
    let scratch_at = r.mem.alloc(16);
    // (the song table and, after it, the authoring string's pointer)
    let table_len = SONGS + 4 * SONG_SIZE - GLOBALS + 4;
    let mut g = Rng(0x9a7f_1d00_0000_0001);
    let mut ops = [0u32; 5];
    let mut markers = 0usize;
    for (name, data) in &songs {
        // a fresh table, the graph, registered on both sides
        r.mem.write(GLOBALS, &vec![0u8; (table_len - 4) as usize]).unwrap();
        r.mem.write_u8(GLOBALS + 0xb, 0xff);
        // (the arena zeroed: a small graph's lookups read past its end)
        r.mem.write(graph_at, &vec![0u8; 0x40000]).unwrap();
        r.mem.write(graph_at, data).unwrap();
        let regions = |r: &mut Runner| Mem {
            regions: vec![
                (GLOBALS, read(r, GLOBALS, table_len)),
                (graph_at, read(r, graph_at, 0x40000)),
                (tracks_at, read(r, tracks_at, TRACK_SIZE * 24)),
                (author & !3, read(r, author & !3, 8)),
            ],
        };
        let mut pf = Pf { mem: regions(&mut r), marker: None };
        r.cpu.set_gpr(4, graph_at as u64);
        r.call(0x2c0b08).unwrap();
        let ours = pf.register(graph_at);
        assert_eq!(r.cpu.gpr(2) as i64, ours, "{name}: register");
        assert!(regions(&mut r) == pf.mem, "{name}: after register");
        let slot = data[0xc] as u32;
        let rec = SONGS + slot * SONG_SIZE;
        let nodes = u16::from_le_bytes([data[0x12], data[0x13]]) as u32;
        let (ntracks, rows) = (data[0xd] as u32, data[0xf] as u32);
        for case in 0..3000 {
            // the tracks: records with random fields, some missing; the song selected
            for k in 0..24u32 {
                let t = tracks_at + TRACK_SIZE * k;
                let mut b: Vec<u8> = (0..TRACK_SIZE).map(|_| g.next() as u8).collect();
                b[0] = k as u8;
                let cnt = g.below(4) as u8;
                b[5] = if g.below(4) == 0 { 0x80 | cnt } else { cnt };
                let lp = if g.below(2) == 0 { 0xffff } else { g.below(nodes) as u16 };
                b[0xc..0xe].copy_from_slice(&lp.to_le_bytes());
                let cur = if g.below(3) == 0 { 0xffff } else { g.below(nodes) as u16 };
                b[0x16..0x18].copy_from_slice(&cur.to_le_bytes());
                r.mem.write(t, &b).unwrap();
                let present = k < ntracks.max(1) || g.below(6) == 0;
                r.mem.write_u32(rec + 0x174 + 4 * k, if present { t } else { 0 });
            }
            r.mem.write_u32(GLOBALS, rec);
            r.mem.write_u8(GLOBALS + 0xb, slot as u8);
            let with_marker = g.below(2) == 0;
            r.mem.write_u32(GLOBALS + 4, if with_marker { MARKER } else { 0 });
            let mut pf = Pf { mem: regions(&mut r), marker: None };
            calls.borrow_mut().clear();
            let mut ours_calls: Vec<(i8, i16)> = vec![];
            let mut push = |s: i8, n: i16| ours_calls.push((s, n));
            if with_marker {
                pf.marker = Some(&mut push);
            }
            let op = g.below(5) as usize;
            ops[op] += 1;
            let what = format!("{name} case {case} op {op}");
            match op {
                0 => {
                    let (node, var) = (g.below(nodes) as u16, g.next() as u64);
                    r.cpu.set_gpr(4, node as i16 as i64 as u64);
                    r.cpu.set_gpr(5, var);
                    r.call(0x2bf038).unwrap();
                    assert_eq!(r.cpu.gpr(2) as u16, pf.branch(node as i16, var), "{what}: branch");
                }
                1 => {
                    let router = if g.below(4) == 0 { -1i64 } else { g.below(nodes) as i64 };
                    let v = if g.below(4) == 0 { 0xffff_ffff } else { g.below(nodes) };
                    r.cpu.set_gpr(4, router as u64);
                    r.cpu.set_gpr(5, v as u64);
                    r.call(0x2bf290).unwrap();
                    assert_eq!(r.cpu.gpr(2) as u32, pf.route(router, v), "{what}: route");
                }
                2 => {
                    let router = if g.below(4) == 0 { 0xffff } else { g.below(nodes) as u16 };
                    let (node, var) = (g.below(nodes) as u16, g.next() as u8);
                    r.cpu.set_gpr(4, router as i16 as i64 as u64);
                    r.cpu.set_gpr(5, node as i16 as i64 as u64);
                    r.cpu.set_gpr(6, var as u64);
                    r.call(0x2bf108).unwrap();
                    assert_eq!(r.cpu.gpr(2) as u16, pf.walk(router, node, var), "{what}: walk");
                }
                3 => {
                    let (event, track) = (g.below(rows) as i16, g.below(ntracks.max(1)) as i16);
                    let scratch = scratch_at;
                    r.cpu.set_gpr(4, event as i64 as u64);
                    r.cpu.set_gpr(5, track as i64 as u64);
                    r.cpu.set_gpr(6, scratch as u64);
                    r.call(0x2bf320).unwrap();
                    let mut o = 0u32;
                    assert_eq!(r.cpu.gpr(2) as i16, pf.event_entry(event, track, &mut o), "{what}: cell");
                    assert_eq!(r.mem.read_u32(scratch), o, "{what}: entry");
                }
                _ => {
                    let event = g.below(rows) as i16;
                    let t = tracks_at + TRACK_SIZE * g.below(ntracks.max(1));
                    let scratch = scratch_at;
                    let pre = g.next();
                    r.mem.write_u32(scratch, pre);
                    r.cpu.set_gpr(4, event as i64 as u64);
                    r.cpu.set_gpr(5, t as u64);
                    r.cpu.set_gpr(6, scratch as u64);
                    r.call(0x2bf490).unwrap();
                    let mut o = pre;
                    assert_eq!(r.cpu.gpr(2) as u16, pf.event_target(event, t, &mut o), "{what}: target");
                    assert_eq!(r.mem.read_u32(scratch), o, "{what}: kept entry");
                }
            }
            let after = regions(&mut r);
            drop(pf.marker.take());
            if after != pf.mem {
                let mut d = vec![];
                for ((b, x), (_, y)) in after.regions.iter().zip(&pf.mem.regions) {
                    for (k, (p, q)) in x.iter().zip(y).enumerate() {
                        if p != q && d.len() < 12 {
                            d.push(format!("{:#x}: game {p:#x} ours {q:#x}", b + k as u32));
                        }
                    }
                }
                panic!("{what}: memory
{}", d.join("
"));
            }
            assert_eq!(*calls.borrow(), ours_calls, "{what}: markers");
            markers += ours_calls.len();
        }
    }
    eprintln!("{} songs, ops {ops:?}, {markers} markers passed", songs.len());
    assert!(0 < markers);
}

#[derive(Clone, Debug, PartialEq)]
enum Sv {
    Time,
    Pos(u32),
    Play(u32, i64, u32, u32, u8, u8, u8),
    Stop(u32),
    Fade(u32),
    Vol(u32, i32),
    VolMs(u32, i32, i32),
    Done(u32),
    GetPos(u32),
    Release(u32),
}

#[derive(Clone, Default)]
struct Streams {
    now: u32,
    pos: Vec<i32>,
    handle: i32,
    done: bool,
    getpos: (i32, i32),
    log: Vec<Sv>,
}

impl PfEnv for Streams {
    fn time_ms(&mut self) -> u32 {
        self.log.push(Sv::Time);
        self.now
    }
    fn position_ms(&mut self, slot: u32) -> i32 {
        self.log.push(Sv::Pos(slot));
        self.pos[slot as usize % self.pos.len()]
    }
    fn play(&mut self, slot: u32, start: i64, name: u32, offset: u32, volume: u8, beat: u8, immediate: u8) -> i32 {
        self.log.push(Sv::Play(slot, start, name, offset, volume, beat, immediate));
        self.handle
    }
    fn stop(&mut self, slot: u32) {
        self.log.push(Sv::Stop(slot));
    }
    fn fade(&mut self, slot: u32) {
        self.log.push(Sv::Fade(slot));
    }
    fn set_volume(&mut self, slot: u32, volume: i32, ms: Option<i32>) {
        self.log.push(match ms {
            Some(m) => Sv::VolMs(slot, volume, m),
            None => Sv::Vol(slot, volume),
        });
    }
    fn is_done(&mut self, slot: u32) -> bool {
        self.log.push(Sv::Done(slot));
        self.done
    }
    fn get_position(&mut self, slot: u32) -> (i32, i32, i32) {
        self.log.push(Sv::GetPos(slot));
        (self.handle, self.getpos.0, self.getpos.1)
    }
    fn release(&mut self, slot: u32) {
        self.log.push(Sv::Release(slot));
    }
}

const TRACK_BLOCK: u32 = 0x140;

#[test]
fn scheduler_is_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let Ok(dir) = std::env::var("TRICKY_MUSIC_DIR") else { return };
    let mut songs: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "mpf"))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .collect();
    songs.sort();
    let w = Rc::new(RefCell::new(Streams::default()));
    macro_rules! s {
        ($addr:expr, |$c:ident, $m:ident, $x:ident| $body:expr) => {{
            let w = w.clone();
            r.stub($addr, move |$c, $m| {
                let mut $x = w.borrow_mut();
                $body
            });
        }};
    }
    s!(0x223158, |c, _m, x| {
        x.log.push(Sv::Time);
        let v = x.now;
        c.set_gpr(2, v as u64)
    });
    s!(0x2235a0, |c, _m, x| {
        let slot = c.gpr(4) as u32;
        x.log.push(Sv::Pos(slot));
        let v = x.pos[slot as usize % x.pos.len()];
        c.set_gpr(2, v as i64 as u64)
    });
    s!(0x223950, |c, _m, x| {
        x.log.push(Sv::Play(c.gpr(4) as u32, c.gpr(5) as i64, c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u8, c.gpr(9) as u8, c.gpr(10) as u8));
        let v = x.handle;
        c.set_gpr(2, v as i64 as u64)
    });
    s!(0x223c40, |c, _m, x| x.log.push(Sv::Stop(c.gpr(4) as u32)));
    s!(0x223ac0, |c, _m, x| x.log.push(Sv::Fade(c.gpr(4) as u32)));
    s!(0x2234e0, |c, _m, x| {
        x.log.push(Sv::Done(c.gpr(4) as u32));
        let v = x.done;
        c.set_gpr(2, v as u64)
    });
    s!(0x223780, |c, m, x| {
        x.log.push(Sv::GetPos(c.gpr(4) as u32));
        let (a, b) = x.getpos;
        m.write_u32(c.gpr(5) as u32, a as u32);
        m.write_u32(c.gpr(6) as u32, b as u32);
        let h = x.handle;
        c.set_gpr(2, h as i64 as u64)
    });
    s!(0x223a90, |c, _m, x| x.log.push(Sv::Release(c.gpr(4) as u32)));
    let vol_log = w.clone();
    let three: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let th = three.clone();
    r.stub(0x223b70, move |c, _| {
        let ev = if *th.borrow() { Sv::VolMs(c.gpr(4) as u32, c.gpr(5) as i32, c.gpr(6) as i32) } else { Sv::Vol(c.gpr(4) as u32, c.gpr(5) as i32) };
        vol_log.borrow_mut().log.push(ev);
    });
    for a in [0x2beca0u32, 0x2bed58, 0x2bf038, 0x2bf108, 0x2bf290, 0x2bf320, 0x2bf490, 0x2befd8, 0x2bfa78, 0x2bfe28, 0x2c00d0, 0x2bfbb8, 0x2f5480, 0x2f54e8, 0x2f5ab8, 0x2f5be8, 0x2f5c88, 0x2f5ec8, 0x2f5f20, 0x2f5f88, 0x2f6230, 0x2f6398, 0x2f64b0, 0x2f6500, 0x2f65b8, 0x2f6650, 0x2f66f0, 0x2f6728] {
        r.allow(a);
    }
    let graph_at = r.mem.alloc(0x40000);
    let tracks_at = r.mem.alloc(TRACK_BLOCK * 24);
    let author = r.mem.read_u32(AUTHOR);
    let table_len = SONGS + 4 * SONG_SIZE - GLOBALS + 4;
    let mut g = Rng(0x5c4e_d000_0000_0001);
    let mut ops = [0u32; 8];
    let mut traps = 0;
    let mut endless = 0;
    let mut slow = 0;
    let mut seen = std::collections::BTreeMap::new();
    for (name, data) in &songs {
        r.mem.write(GLOBALS, &vec![0u8; (table_len - 4) as usize]).unwrap();
        r.mem.write_u8(GLOBALS + 0xb, 0xff);
        r.mem.write(graph_at, &vec![0u8; 0x40000]).unwrap();
        r.mem.write(graph_at, data).unwrap();
        r.cpu.set_gpr(4, graph_at as u64);
        r.call(0x2c0b08).unwrap();
        let slot = data[0xc] as u32;
        let rec = SONGS + slot * SONG_SIZE;
        let nodes = u16::from_le_bytes([data[0x12], data[0x13]]) as u32;
        let (ntracks, rows) = (data[0xd] as u32, data[0xf] as u32);
        let node_rec = |r: &mut Runner, n: u32| {
            let offs = r.mem.read_u32(rec + 0x18);
            let o = r.mem.read_u32((offs + 2 * n) & !3) >> (16 * (((offs + 2 * n) >> 1) & 1));
            graph_at.wrapping_add(((o as u16 as i16 as i32) * 4) as u32)
        };
        for case in 0..2500 {
            for k in 0..24u32 {
                let t = tracks_at + TRACK_BLOCK * k;
                let mut b: Vec<u8> = (0..TRACK_BLOCK).map(|_| g.next() as u8).collect();
                b[0] = k as u8;
                b[1] = g.next() as u8;
                b[2] = 0;
                b[4] = [100u8, 50, 127, 0][g.below(4) as usize];
                b[5] = g.below(4) as u8;
                b[6] = [0u8, 1, 2, 3, 0x7f][g.below(5) as usize];
                b[8] = 1 + g.below(8) as u8;
                b[9] = g.below(b[8] as u32) as u8;
                let lat = [0u16, 30, 49, 50, 200][g.below(5) as usize];
                b[10..12].copy_from_slice(&lat.to_le_bytes());
                let lp = if g.below(2) == 0 { 0xffff } else { g.below(nodes) as u16 };
                b[0xc..0xe].copy_from_slice(&lp.to_le_bytes());
                let ev = if g.below(2) == 0 { 0xffff } else { g.below(nodes) as u16 };
                b[0xe..0x10].copy_from_slice(&ev.to_le_bytes());
                let marker = if g.below(3) == 0 { 0 } else { node_rec(&mut r, g.below(nodes)) };
                b[0x10..0x14].copy_from_slice(&marker.to_le_bytes());
                b[0x15] &= 0x7f;
                let cur = if g.below(5) == 0 { 0xffff } else { g.below(nodes) as u16 };
                b[0x16..0x18].copy_from_slice(&cur.to_le_bytes());
                let tgt = g.below(nodes) as u16;
                b[0x1a..0x1c].copy_from_slice(&tgt.to_le_bytes());
                let at = [0i32, 0, 100, -5][g.below(4) as usize];
                b[0x24..0x28].copy_from_slice(&at.to_le_bytes());
                let slotid = g.below(6);
                b[0x2c..0x30].copy_from_slice(&slotid.to_le_bytes());
                b[0x34..0x3c].copy_from_slice(b"track\0\0\0");
                let ring = t + 0xd8;
                b[0xd4..0xd8].copy_from_slice(&ring.to_le_bytes());
                r.mem.write(t, &b).unwrap();
                let present = k < ntracks.max(1) || g.below(6) == 0;
                r.mem.write_u32(rec + 0x174 + 4 * k, if present { t } else { 0 });
            }
            // a marker record with chosen beat bytes (in the last track block's spare bytes)
            let synth = tracks_at + TRACK_BLOCK * 23 + 0x100;
            r.mem.write_u8(synth + 6, [0u8, 1, 4, 0x7f, 0xff][g.below(5) as usize]);
            r.mem.write_u8(synth + 7, g.below(4) as u8);
            r.mem.write_u8(synth + 8, g.below(4) as u8);
            r.mem.write_u32(GLOBALS, rec);
            r.mem.write_u8(GLOBALS + 0xb, slot as u8);
            r.mem.write_u32(GLOBALS + 4, 0);
            let lead = g.below(ntracks.max(1) + 1) as i32 - 1;
            r.mem.write_u32(rec, lead as u32);
            let ev: i32 = if g.below(4) == 0 { !(g.below(nodes) as i32) } else { g.below(rows.max(1)) as i32 };
            let w34 = (r.mem.read_u32(rec + 0x34) & 0xffff_0000) | (ev as u32 & 0xffff);
            r.mem.write_u32(rec + 0x34, w34);
            r.mem.write_u8(rec + 0x37, (g.below(3) == 0) as u8);
            r.mem.write_u32(rec + 0x3c, g.below(5000));
            r.mem.write_u32(rec + 0x40, g.below(5000));
            r.mem.write_u32(rec + 0x44, if g.below(2) == 0 { 0 } else { g.next() });
            {
                let mut x = w.borrow_mut();
                *x = Streams {
                    now: g.below(100_000),
                    pos: (0..6).map(|_| [0i32, -1, 500, 3000][g.below(4) as usize]).collect(),
                    handle: if g.below(4) == 0 { -1 } else { g.below(50) as i32 },
                    done: g.below(2) == 0,
                    getpos: ([-1i32, 0, 400, 2000][g.below(4) as usize], [-1i32, 0, 300, 5000][g.below(4) as usize]),
                    log: vec![],
                };
            }
            let regions = |r: &mut Runner| Mem {
                regions: vec![
                    (GLOBALS, read(r, GLOBALS, table_len)),
                    (graph_at, read(r, graph_at, 0x40000)),
                    (tracks_at, read(r, tracks_at, TRACK_BLOCK * 24)),
                    (author & !3, read(r, author & !3, 8)),
                ],
            };
            let mut pf = Pf { mem: regions(&mut r), marker: None };
            let mut ours = w.borrow().clone();
            let op = g.below(8) as usize;
            ops[op] += 1;
            *three.borrow_mut() = op == 1;
            let what = format!("{name} case {case} op {op}");
            let k = g.below(ntracks.max(1));
            let t = tracks_at + TRACK_BLOCK * k;
            let (theirs, ours_ret) = match op {
                0 => {
                    let mask = g.next() & 0x0f00_0000;
                    r.cpu.set_gpr(4, mask as u64);
                    let res = r.call(0x2bee88).map(|_| r.cpu.gpr(2) as i64);
                    (res, std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pf.count_active_tracks(mask) as i64)))
                }
                1 => {
                    let mask = g.next();
                    r.cpu.set_gpr(4, mask as u64);
                    let res = r.call(0x2bef08).map(|_| 0);
                    (res, std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        pf.fade_tracks(&mut ours, mask);
                        0
                    })))
                }
                2 => {
                    let n = g.below(nodes + 4) as i64 - 2;
                    r.cpu.set_gpr(4, n as u64);
                    let res = r.call(0x2befd8).map(|_| r.cpu.gpr(2) as u32 as i64);
                    (res, std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pf.segment_offset(n) as i64)))
                }
                3 => {
                    let n = g.below(nodes) as i16;
                    r.cpu.set_gpr(4, n as i64 as u64);
                    let res = r.call(0x2bfa78).map(|_| 0);
                    (res, std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        pf.release_waiting(&mut ours, n);
                        0
                    })))
                }
                4 => {
                    let im = g.below(2) as u8;
                    r.cpu.set_gpr(4, t as u64);
                    r.cpu.set_gpr(5, im as u64);
                    let res = r.call(0x2bfe28).map(|_| r.cpu.gpr(2) as i32 as i64);
                    (res, std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pf.start_segment(&mut ours, t, im) as i64)))
                }
                5 => {
                    r.cpu.set_gpr(4, k as u64);
                    let res = r.call(0x2bfbb8).map(|_| 0);
                    (res, std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        pf.advance(&mut ours, k);
                        0
                    })))
                }
                6 => {
                    let n = node_rec(&mut r, g.below(nodes));
                    let marker = if g.below(2) == 0 { synth } else { node_rec(&mut r, g.below(nodes)) };
                    let out = t + 0x24;
                    r.cpu.set_gpr(4, n as u64);
                    r.cpu.set_gpr(5, marker as u64);
                    r.cpu.set_gpr(6, out as u64);
                    let res = r.call(0x2c00d0).map(|_| r.cpu.gpr(2) as i32 as i64);
                    (res, std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pf.sync_offset(&mut ours, n, marker, out) as i64)))
                }
                _ => {
                    r.cpu.set_gpr(4, k as u64);
                    let res = r.call(0x2bf508).map(|_| r.cpu.gpr(2) as u16 as i64);
                    (res, std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pf.next_segment(&mut ours, k as i32) as i64)))
                }
            };
            match (theirs, ours_ret) {
                (Err(r5900::Error::Trap { .. }), Err(_)) => {
                    traps += 1;
                    continue;
                }
                // (random lengths can make the sync's wrap-up loop crawl past the runner's step limit)
                (Err(r5900::Error::TooLong { .. }), Ok(_)) if op == 6 => {
                    slow += 1;
                    continue;
                }
                (Err(r5900::Error::TooLong { .. }), Err(_)) => {
                    endless += 1;
                    continue;
                }
                // (the endless recursion: the game runs off its stack)
                (Err(_), Err(_)) => {
                    endless += 1;
                    continue;
                }
                (Ok(a), Ok(b)) => {
                    assert_eq!(a, b, "{what}: result");
                    let key = match op {
                        6 => format!("sync {}", a.clamp(-1, 3)),
                        7 => format!("next {}", if a == 0xffff { "none" } else { "node" }),
                        4 => format!("start {}", if a < 0 { "failed" } else { "played" }),
                        _ => String::new(),
                    };
                    *seen.entry(key).or_insert(0u32) += 1;
                }
                (a, b) => panic!("{what}: game {a:?}, ours {}", if b.is_ok() { "ok" } else { "panicked" }),
            }
            assert_eq!(w.borrow().log, ours.log, "{what}: stream calls");
            let after = regions(&mut r);
            if after != pf.mem {
                let mut d = vec![];
                for ((b, x), (_, y)) in after.regions.iter().zip(&pf.mem.regions) {
                    for (k, (p, q)) in x.iter().zip(y).enumerate() {
                        if p != q && d.len() < 12 {
                            d.push(format!("{:#x}: game {p:#x} ours {q:#x}", b + k as u32));
                        }
                    }
                }
                panic!("{what}: memory\n{}", d.join("\n"));
            }
        }
    }
    eprintln!("ops {ops:?}, {traps} traps and {endless} endless recursions on both sides, {slow} syncs past the step limit
{seen:?}");
}

#[derive(Clone, Debug, PartialEq)]
enum Dv {
    S(Sv),
    Free(u32),
    StopDriver,
    Pitch(u32, u8),
    FadeTo(u32, i32, u64),
    Overhead,
    Init,
    Open(u32, u32, i32, u32, u32, i32),
}

#[derive(Clone, Default)]
struct Driver {
    s: Streams,
    overhead: i32,
    slot: i32,
    log: Vec<Dv>,
}

impl PfEnv for Driver {
    fn time_ms(&mut self) -> u32 {
        self.log.push(Dv::S(Sv::Time));
        self.s.now
    }
    fn position_ms(&mut self, slot: u32) -> i32 {
        self.log.push(Dv::S(Sv::Pos(slot)));
        self.s.pos[slot as usize % self.s.pos.len()]
    }
    fn play(&mut self, slot: u32, start: i64, name: u32, offset: u32, volume: u8, beat: u8, immediate: u8) -> i32 {
        self.log.push(Dv::S(Sv::Play(slot, start, name, offset, volume, beat, immediate)));
        self.s.handle
    }
    fn stop(&mut self, slot: u32) {
        self.log.push(Dv::S(Sv::Stop(slot)));
    }
    fn fade(&mut self, slot: u32) {
        self.log.push(Dv::S(Sv::Fade(slot)));
    }
    fn set_volume(&mut self, slot: u32, volume: i32, ms: Option<i32>) {
        self.log.push(Dv::S(match ms {
            Some(m) => Sv::VolMs(slot, volume, m),
            None => Sv::Vol(slot, volume),
        }));
    }
    fn is_done(&mut self, slot: u32) -> bool {
        self.log.push(Dv::S(Sv::Done(slot)));
        self.s.done
    }
    fn get_position(&mut self, slot: u32) -> (i32, i32, i32) {
        self.log.push(Dv::S(Sv::GetPos(slot)));
        (self.s.handle, self.s.getpos.0, self.s.getpos.1)
    }
    fn release(&mut self, slot: u32) {
        self.log.push(Dv::S(Sv::Release(slot)));
    }
}

impl PfDriver for Driver {
    fn free_track(&mut self, slot: u32) {
        self.log.push(Dv::Free(slot));
    }
    fn stop_driver(&mut self) {
        self.log.push(Dv::StopDriver);
    }
    fn set_pitch(&mut self, slot: u32, pitch: u8) {
        self.log.push(Dv::Pitch(slot, pitch));
    }
    fn fade_to(&mut self, slot: u32, volume: i32, ms: u64) {
        self.log.push(Dv::FadeTo(slot, volume, ms));
    }
    fn track_overhead(&mut self) -> i32 {
        self.log.push(Dv::Overhead);
        self.overhead
    }
    fn init_driver(&mut self) {
        self.log.push(Dv::Init);
    }
    fn open_song(&mut self, name: u32, a: u32, n: i32, b: u32, buffer: u32, size: i32) -> i32 {
        self.log.push(Dv::Open(name, a, n, b, buffer, size));
        self.slot
    }
}

#[test]
fn callbacks_and_controls_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let Ok(dir) = std::env::var("TRICKY_MUSIC_DIR") else { return };
    let mut songs: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "mpf"))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .collect();
    songs.sort();
    let w = Rc::new(RefCell::new(Driver::default()));
    let fade_has_args: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    macro_rules! s {
        ($addr:expr, |$c:ident, $m:ident, $x:ident| $body:expr) => {{
            let w = w.clone();
            r.stub($addr, move |$c, $m| {
                let mut $x = w.borrow_mut();
                $body
            });
        }};
    }
    s!(0x223158, |c, _m, x| {
        x.log.push(Dv::S(Sv::Time));
        let v = x.s.now;
        c.set_gpr(2, v as u64)
    });
    s!(0x2235a0, |c, _m, x| {
        let slot = c.gpr(4) as u32;
        x.log.push(Dv::S(Sv::Pos(slot)));
        let v = x.s.pos[slot as usize % x.s.pos.len()];
        c.set_gpr(2, v as i64 as u64)
    });
    s!(0x223950, |c, _m, x| {
        x.log.push(Dv::S(Sv::Play(c.gpr(4) as u32, c.gpr(5) as i64, c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u8, c.gpr(9) as u8, c.gpr(10) as u8)));
        let v = x.s.handle;
        c.set_gpr(2, v as i64 as u64)
    });
    s!(0x223c40, |c, _m, x| x.log.push(Dv::S(Sv::Stop(c.gpr(4) as u32))));
    let fa = fade_has_args.clone();
    s!(0x223ac0, |c, _m, x| {
        let ev = if *fa.borrow() { Dv::FadeTo(c.gpr(4) as u32, c.gpr(5) as i32, c.gpr(6)) } else { Dv::S(Sv::Fade(c.gpr(4) as u32)) };
        x.log.push(ev)
    });
    s!(0x2234e0, |c, _m, x| {
        x.log.push(Dv::S(Sv::Done(c.gpr(4) as u32)));
        let v = x.s.done;
        c.set_gpr(2, v as u64)
    });
    s!(0x223780, |c, m, x| {
        x.log.push(Dv::S(Sv::GetPos(c.gpr(4) as u32)));
        let (a, b) = x.s.getpos;
        m.write_u32(c.gpr(5) as u32, a as u32);
        m.write_u32(c.gpr(6) as u32, b as u32);
        let h = x.s.handle;
        c.set_gpr(2, h as i64 as u64)
    });
    s!(0x223a90, |c, _m, x| x.log.push(Dv::S(Sv::Release(c.gpr(4) as u32))));
    // (FUN_00223b70 is also called with a third argument, 500, when the volumes ease in)
    s!(0x223b70, |c, _m, x| {
        let ev = if c.gpr(6) == 500 { Sv::VolMs(c.gpr(4) as u32, c.gpr(5) as i32, 500) } else { Sv::Vol(c.gpr(4) as u32, c.gpr(5) as i32) };
        x.log.push(Dv::S(ev))
    });
    s!(0x223450, |c, _m, x| x.log.push(Dv::Free(c.gpr(4) as u32)));
    s!(0x223710, |_c, _m, x| x.log.push(Dv::StopDriver));
    s!(0x223b78, |c, _m, x| x.log.push(Dv::Pitch(c.gpr(4) as u32, c.gpr(5) as u8)));
    s!(0x2231a8, |c, _m, x| {
        x.log.push(Dv::Overhead);
        let v = x.overhead;
        c.set_gpr(2, v as i64 as u64)
    });
    s!(0x223648, |_c, _m, x| x.log.push(Dv::Init));
    s!(0x2231d8, |c, m, x| {
        let size = m.read_u32(c.gpr(29) as u32 + 0) as i32;
        let _ = size;
        x.log.push(Dv::Open(c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as i32, c.gpr(7) as u32, c.gpr(8) as u32, c.gpr(9) as i32));
        let v = x.slot;
        c.set_gpr(2, v as i64 as u64)
    });
    // the C library: strcpy, strlen, sprintf("%-8s")
    r.stub(0x2fd14c, |c, m| {
        let (d, s) = (c.gpr(4) as u32, c.gpr(5) as u32);
        for k in 0.. {
            let b = (m.read_u32((s + k) & !3) >> (8 * ((s + k) & 3))) as u8;
            m.write_u8(d + k, b);
            if b == 0 {
                break;
            }
        }
        c.set_gpr(2, d as u64);
    });
    r.stub(0x2fd260, |c, m| {
        let s = c.gpr(4) as u32;
        let mut n = 0;
        while (m.read_u32((s + n) & !3) >> (8 * ((s + n) & 3))) as u8 != 0 {
            n += 1;
        }
        c.set_gpr(2, n as u64);
    });
    r.stub(0x2fca28, |c, m| {
        let (d, s) = (c.gpr(4) as u32, c.gpr(6) as u32);
        let mut v = vec![];
        for k in 0.. {
            let b = (m.read_u32((s + k) & !3) >> (8 * ((s + k) & 3))) as u8;
            if b == 0 {
                break;
            }
            v.push(b);
        }
        while v.len() < 8 {
            v.push(b' ');
        }
        for (k, b) in v.iter().enumerate() {
            m.write_u8(d + k as u32, *b);
        }
        m.write_u8(d + v.len() as u32, 0);
        c.set_gpr(2, v.len() as u64);
    });
    for a in [0x2beca0u32, 0x2bed58, 0x2bf038, 0x2bf108, 0x2bf290, 0x2bf320, 0x2bf490, 0x2befd8, 0x2bfa78, 0x2bfe28, 0x2c00d0, 0x2bfbb8, 0x2bf508, 0x2bee88, 0x2bef08, 0x2bedf0, 0x2c1cc0, 0x2c1b98, 0x2c1a38] {
        r.allow(a);
    }
    for a in [0x2f5480u32, 0x2f54e8, 0x2f5ab8, 0x2f5be8, 0x2f5c88, 0x2f5ec8, 0x2f5f20, 0x2f5f88, 0x2f6230, 0x2f6398, 0x2f64b0, 0x2f6500, 0x2f65b8, 0x2f6650, 0x2f66f0, 0x2f6728] {
        r.allow(a);
    }
    let graph_at = r.mem.alloc(0x40000);
    let tracks_at = r.mem.alloc(TRACK_BLOCK * 24);
    let spare = r.mem.alloc(0x400);
    let author = r.mem.read_u32(AUTHOR);
    let table_len = SONGS + 4 * SONG_SIZE - GLOBALS + 4;
    let mut g = Rng(0xc0_47_801_7000_0001);
    let mut ops = [0u32; 19];
    let mut odd = 0;
    for (name, data) in &songs {
        r.mem.write(GLOBALS, &vec![0u8; (table_len - 4) as usize]).unwrap();
        r.mem.write_u8(GLOBALS + 0xb, 0xff);
        r.mem.write(graph_at, &vec![0u8; 0x40000]).unwrap();
        r.mem.write(graph_at, data).unwrap();
        r.cpu.set_gpr(4, graph_at as u64);
        r.call(0x2c0b08).unwrap();
        let slot = data[0xc] as u32;
        let rec = SONGS + slot * SONG_SIZE;
        let nodes = u16::from_le_bytes([data[0x12], data[0x13]]) as u32;
        let (ntracks, rows) = (data[0xd] as u32, data[0xf] as u32);
        for case in 0..1500 {
            for k in 0..24u32 {
                let t = tracks_at + TRACK_BLOCK * k;
                let mut b: Vec<u8> = (0..TRACK_BLOCK).map(|_| g.next() as u8).collect();
                b[0] = k as u8;
                b[2] = if g.below(6) == 0 { 1 } else { 0 };
                b[4] = [100u8, 50, 127, 0][g.below(4) as usize];
                b[5] = g.below(4) as u8;
                b[6] = [0u8, 1, 2, 3][g.below(4) as usize];
                b[8] = 1 + g.below(8) as u8;
                b[9] = g.below(b[8] as u32) as u8;
                let lat = [0u16, 30, 49, 50, 200][g.below(5) as usize];
                b[10..12].copy_from_slice(&lat.to_le_bytes());
                let lp = if g.below(2) == 0 { 0xffff } else { g.below(nodes) as u16 };
                b[0xc..0xe].copy_from_slice(&lp.to_le_bytes());
                let ev = if g.below(2) == 0 { 0xffff } else { g.below(nodes) as u16 };
                b[0xe..0x10].copy_from_slice(&ev.to_le_bytes());
                let marker = if g.below(3) == 0 { 0 } else { graph_at + 0x24 };
                b[0x10..0x14].copy_from_slice(&marker.to_le_bytes());
                b[0x15] &= 0x3f;
                let cur = if g.below(5) == 0 { 0xffff } else { g.below(nodes) as u16 };
                b[0x16..0x18].copy_from_slice(&cur.to_le_bytes());
                let tgt = g.below(nodes) as u16;
                b[0x1a..0x1c].copy_from_slice(&tgt.to_le_bytes());
                let at = [0i32, 0, 100, -5][g.below(4) as usize];
                b[0x24..0x28].copy_from_slice(&at.to_le_bytes());
                let at2 = [0i32, 0, 50, 900000][g.below(4) as usize];
                b[0x28..0x2c].copy_from_slice(&at2.to_le_bytes());
                b[0x2c..0x30].copy_from_slice(&g.below(6).to_le_bytes());
                b[0x34..0x3c].copy_from_slice(b"track\0\0\0");
                let ring = t + 0xd8;
                b[0xd4..0xd8].copy_from_slice(&ring.to_le_bytes());
                // ring handles that the stream can name
                for q in 0..8usize {
                    let h = g.below(6) as i32 - 1;
                    b[0xd8 + 0xc * q + 8..0xd8 + 0xc * q + 12].copy_from_slice(&h.to_le_bytes());
                }
                r.mem.write(t, &b).unwrap();
                let present = k < ntracks.max(1) || g.below(6) == 0;
                r.mem.write_u32(rec + 0x174 + 4 * k, if present { t } else { 0 });
            }
            // the song record's event queue
            for j in 0..16u32 {
                let e = rec + 0x34 + 0x14 * j;
                let ev: i32 = if g.below(4) == 0 { !(g.below(nodes) as i32) } else { g.below(rows.max(1)) as i32 };
                r.mem.write_u16(e, ev as u16);
                r.mem.write_u8(e + 2, g.below(4) as u8);
                r.mem.write_u8(e + 3, (g.below(3) == 0) as u8);
                r.mem.write_u32(e + 4, g.below(25));
                r.mem.write_u32(e + 8, g.below(5000));
                r.mem.write_u32(e + 0xc, g.below(5000));
                let filled = j < g.below(17);
                r.mem.write_u32(e + 0x10, if filled { if g.below(3) == 0 { u32::MAX } else { g.next() | 1 } } else { 0 });
            }
            r.mem.write_u32(rec, g.below(ntracks.max(1) + 1).wrapping_sub(1));
            r.mem.write_u8(GLOBALS + 8, if g.below(12) == 0 { 0 } else { 1 });
            r.mem.write_u8(GLOBALS + 9, if g.below(6) == 0 { 1 } else { 0 });
            r.mem.write_u8(GLOBALS + 0xa, g.below(101) as u8);
            r.mem.write_u8(GLOBALS + 0xb, if g.below(12) == 0 { slot as u8 } else { 0xff });
            r.mem.write_u32(GLOBALS, 0);
            r.mem.write_u32(GLOBALS + 4, 0);
            {
                let mut x = w.borrow_mut();
                *x = Driver {
                    s: Streams {
                        now: g.below(100_000),
                        pos: (0..6).map(|_| [0i32, -1, 500, 3000][g.below(4) as usize]).collect(),
                        handle: if g.below(4) == 0 { -1 } else { g.below(5) as i32 },
                        done: g.below(2) == 0,
                        getpos: ([-1i32, 0, 400, 2000][g.below(4) as usize], [-1i32, 0, 300, 5000][g.below(4) as usize]),
                        log: vec![],
                    },
                    overhead: g.below(0x400) as i32,
                    slot: g.below(6) as i32 - 1,
                    log: vec![],
                };
            }
            let regions = |r: &mut Runner| Mem {
                regions: vec![
                    (GLOBALS, read(r, GLOBALS, table_len)),
                    (graph_at, read(r, graph_at, 0x40000)),
                    (tracks_at, read(r, tracks_at, TRACK_BLOCK * 24)),
                    (spare, read(r, spare, 0x400)),
                    (author & !3, read(r, author & !3, 8)),
                    // (a track with no marker has the sync read its fields at address 0)
                    (0, read(r, 0, 0x100)),
                ],
            };
            let mut pf = Pf { mem: regions(&mut r), marker: None };
            let mut ours = w.borrow().clone();
            let op = g.below(19) as usize;
            ops[op] += 1;
            *fade_has_args.borrow_mut() = op == 16;
            let what = format!("{name} case {case} op {op}");
            let songmask = if g.below(4) == 0 { 0x0f00_0000 } else { 0x0100_0000 << if g.below(3) == 0 { g.below(4) } else { slot } };
            let trackmask = if g.below(3) == 0 { 0xff_ffff } else { g.next() & 0xff_ffff };
            let mask = if g.below(8) == 0 { u32::MAX } else { songmask | trackmask };
            let id = (0x0100_0000 << if g.below(4) == 0 { g.below(4) } else { slot }) | (1 << g.below(24));
            let out = spare;
            macro_rules! go {
                ($f:expr, [$($a:expr),*], $ours:expr) => {{
                    // (the game's callers pass ints: sign-extended in the registers)
                    let args: &[u64] = &[$($a as i32 as i64 as u64),*];
                    for (k, a) in args.iter().enumerate() {
                        r.cpu.set_gpr(4 + k, *a);
                    }
                    let res = r.call($f).map(|_| r.cpu.gpr(2));
                    let o = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $ours));
                    (res, o)
                }};
            }
            let (theirs, oursr): (Result<u64, r5900::Error>, std::thread::Result<i64>) = match op {
                0 => go!(0x2c0488, [], {
                    pf.update(&mut ours);
                    0
                }),
                1 => go!(0x2c0898, [], {
                    pf.beat(&mut ours);
                    0
                }),
                2 => {
                    let (a, b) = (g.below(200), g.below(50));
                    go!(0x2c1068, [mask, a, b], {
                        pf.set_lookahead(mask, a, b);
                        0
                    })
                }
                3 => {
                    let lat = g.below(300) as u16;
                    go!(0x2c1130, [id, lat], pf.set_latency(id, lat) as i64)
                }
                4 => go!(0x2c1168, [id], pf.slot_of(id) as i64),
                5 => go!(0x2c1190, [id, out], pf.playing(&mut ours, id, out) as i64),
                6 => {
                    let v = g.below(200) as u64;
                    go!(0x2c1288, [mask, v], pf.set_variable(mask, v) as i64)
                }
                7 => go!(0x2c1380, [mask], {
                    pf.clear_events(mask);
                    0
                }),
                8 => go!(0x2c1430, [mask], pf.queued_events(mask) as i64),
                9 => {
                    let o = if g.below(2) == 0 { 0 } else { out };
                    go!(0x2c1500, [mask, o], pf.current_event(mask, if o == 0 { None } else { Some(o) }) as i64)
                }
                10 => {
                    let ev = ((if g.below(4) == 0 { g.below(4) } else { slot }) << 8 | g.below(rows.max(1) + 1)) as i16;
                    let prio = [-1i8, 0, 1, 2, 3][g.below(5) as usize];
                    go!(0x2c15c8, [trackmask, ev as i64, prio as i64], pf.post_event(&mut ours, trackmask, ev, prio) as i64)
                }
                11 => {
                    let node = if g.below(6) == 0 { -1i64 } else { g.below(nodes + 2) as i64 };
                    let only = songmask | (1 << g.below(ntracks.max(1)));
                    let imm = g.below(2) as u8;
                    go!(0x2c1890, [only, node, imm], pf.jump_to(only, node, imm) as i64)
                }
                12 => go!(0x2c1a38, [mask], pf.stop_tracks(&mut ours, mask) as i32 as i64),
                13 => {
                    let p = g.below(3) as u8;
                    go!(0x2c1b98, [mask, p], {
                        pf.set_pitch(&mut ours, mask, p);
                        0
                    })
                }
                14 => go!(0x2c1cc0, [mask], {
                    pf.stop_song(&mut ours, mask);
                    0
                }),
                15 => {
                    let v = [-5i8, 0, 50, 100, 120][g.below(5) as usize];
                    go!(0x2c1db8, [mask, v as i64], {
                        pf.set_volume(&mut ours, mask, v);
                        0
                    })
                }
                16 => {
                    let (v, ms) = (g.below(128) as i32, g.below(2000) as u64);
                    go!(0x2c1f10, [mask, v, ms], {
                        pf.fade_to(&mut ours, mask, v, ms);
                        0
                    })
                }
                18 => go!(0x2bedb8, [], {
                    pf.shut_down(&mut ours);
                    0
                }),
                _ => {
                    // a song's track loaded into a spare record (in the last track's block)
                    let track = g.below(24);
                    if g.below(2) == 0 {
                        r.mem.write_u32(rec + 0x174 + 4 * track, 0);
                        pf.mem.w32(rec + 0x174 + 4 * track, 0);
                    }
                    let nm: &[u8] = [&b"data\\audio\\smart.bomb.mus\0"[..], b"x.mus\0", b"a:b/c\\longername.mus\0", b"plain\0"][g.below(4) as usize];
                    let name_at = spare + 0x300;
                    r.mem.write(name_at, nm).unwrap();
                    for (k, b) in nm.iter().enumerate() {
                        pf.mem.w8(name_at + k as u32, *b);
                    }
                    let recp = spare;
                    let (lat, a, n, b, size) = (g.below(300), g.next(), 1 + g.below(8) as i32, g.next(), 0x300 + g.below(0x400) as i32);
                    r.mem.write_u32(0x01ff_0000, size as u32);
                    let song = if g.below(4) == 0 { g.below(5) } else { slot };
                    go!(0x2c0cb0, [song, track, name_at, lat, a, n, b, recp], pf.load_song(&mut ours, song, track, name_at, nm, lat as u16, a, n, b, recp, size) as i64)
                }
            };
            match (theirs, oursr) {
                (Ok(a), Ok(b)) => {
                    let a = match op {
                        3 | 4 | 5 | 6 | 8 | 10 | 11 | 12 | 17 => a as i32 as i64,
                        9 => a as u16 as i64,
                        _ => 0,
                    };
                    assert_eq!(a, b, "{what}: result");
                }
                (Err(_), Err(_)) => {
                    odd += 1;
                    continue;
                }
                (Err(r5900::Error::TooLong { .. }), Ok(_)) => {
                    odd += 1;
                    continue;
                }
                (a, b) => panic!("{what}: game {a:?}, ours {}", if b.is_ok() { "ok" } else { "panicked" }),
            }
            assert_eq!(w.borrow().log, ours.log, "{what}: driver calls");
            let after = regions(&mut r);
            if after != pf.mem {
                let mut d = vec![];
                for ((b, x), (_, y)) in after.regions.iter().zip(&pf.mem.regions) {
                    for (k, (p, q)) in x.iter().zip(y).enumerate() {
                        if p != q && d.len() < 12 {
                            d.push(format!("{:#x}: game {p:#x} ours {q:#x}", b + k as u32));
                        }
                    }
                }
                panic!("{what}: memory\n{}", d.join("\n"));
            }
        }
    }
    eprintln!("ops {ops:?}, {odd} traps / hangs on both sides");
}

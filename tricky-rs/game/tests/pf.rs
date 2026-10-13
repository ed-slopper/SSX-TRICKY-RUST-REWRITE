//! Pathfinder's graph walk (board row G10b, part 1) against the game in the function runner (TRICKY_ELF and
//! TRICKY_MUSIC_DIR, the songs' `.mpf` files out of DATA/AUDIO/MUSIC.BIG; skipped without them): every song graph
//! registered by `PathX_RegisterSong` on both sides, then random branches, routers, walks and event lookups on random
//! track records: the song table, the graph, the tracks and the marker callback's calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::{Mem, Pf, PfEnv, AUTHOR, GLOBALS, SONGS, SONG_SIZE};

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
    fn get_position(&mut self, slot: u32) -> (i32, i32) {
        self.log.push(Sv::GetPos(slot));
        self.getpos
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

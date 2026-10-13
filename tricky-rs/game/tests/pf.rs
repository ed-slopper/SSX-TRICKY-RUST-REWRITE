//! Pathfinder's graph walk (board row G10b, part 1) against the game in the function runner (TRICKY_ELF and
//! TRICKY_MUSIC_DIR, the songs' `.mpf` files out of DATA/AUDIO/MUSIC.BIG; skipped without them): every song graph
//! registered by `PathX_RegisterSong` on both sides, then random branches, routers, walks and event lookups on random
//! track records: the song table, the graph, the tracks and the marker callback's calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::{Mem, Pf, AUTHOR, GLOBALS, SONGS, SONG_SIZE};

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

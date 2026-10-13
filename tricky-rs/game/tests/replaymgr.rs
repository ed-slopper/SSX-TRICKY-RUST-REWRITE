//! The replay manager's keyframe list, seeking and playlists (board row G12d) against the game in the function runner
//! (TRICKY_ELF; skipped without it): random managers (keyframe lists and free lists, pins, highlights, playlists,
//! camera tracks, pad streams) built the same in the runner and in our memory, then random calls, every return and
//! all the memory compared after each.

use r5900::Runner;
use tricky_game::replaymgr::{self as rm, kf, Mem};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

const MGR_LEN: u32 = 0x40a8;
const KEYFRAMES: u32 = 0x3e68;
const H_LEN: u32 = 0x200;
const IN_LEN: u32 = 0x800c;
const LOW: u32 = 0x100;

struct World {
    mgr: u32,
    h: u32,
    inputs: [u32; 4],
}

fn regions(r: &mut Runner, w: &World) -> Vec<(u32, u32)> {
    let _ = r;
    let mut v = vec![(0, LOW), (w.mgr, MGR_LEN), (w.h, H_LEN)];
    for a in w.inputs {
        v.push((a, IN_LEN));
    }
    v
}

fn snapshot(r: &mut Runner, w: &World) -> Mem {
    let mut m = Mem::default();
    for (a, n) in regions(r, w) {
        let mut b = vec![0u8; n as usize];
        r.mem.read(a, &mut b).unwrap();
        m.regions.push((a, b));
    }
    m
}

fn build(g: &mut Rng, r: &mut Runner, w: &World) -> Mem {
    let mut m = snapshot(r, w);
    let mgr = w.mgr;
    for (base, v) in m.regions.iter_mut() {
        if *base != 0 {
            v.iter_mut().for_each(|b| *b = 0);
        }
    }
    let slot = |i: u32| mgr + KEYFRAMES + 0x24 * i;
    // the keyframe list and the free list
    let mut order: Vec<u32> = (0..16).collect();
    for i in (1..16).rev() {
        order.swap(i, g.below(i as u32 + 1) as usize);
    }
    let n = 1 + g.below(16) as usize;
    let (list, free) = order.split_at(n);
    let mut frame = g.below(50);
    for (j, &i) in list.iter().enumerate() {
        let k = slot(i);
        m.w32(k + kf::FRAME, frame);
        frame += [0, 1, 30, 60, 200][g.below(5) as usize];
        m.w32(k + kf::PREV, if j > 0 { slot(list[j - 1]) } else { 0 });
        m.w32(k + kf::NEXT, if j + 1 < n { slot(list[j + 1]) } else { 0 });
        m.w32(k + kf::PINS, if g.below(3) == 0 { 1 + g.below(2) } else { 0 });
        m.w32(k + kf::USED, (g.below(5) == 0) as u32);
        for off in [4, 8, 0xc] {
            m.w32(k + off, g.next());
        }
    }
    for (j, &i) in free.iter().enumerate() {
        m.w32(slot(i) + kf::NEXT, if j + 1 < free.len() { slot(free[j + 1]) } else { 0 });
        m.w32(slot(i) + kf::FRAME, g.next());
    }
    m.w32(mgr, slot(list[0]));
    m.w32(mgr + 4, if free.is_empty() { 0 } else { slot(free[0]) });
    m.w32(mgr + 8, 16);
    let pick = |g: &mut Rng| slot(list[g.below(n as u32) as usize]);
    m.w32(mgr + 0x10, if g.below(6) == 0 { 0 } else { slot(list[n - 1]) });
    m.w32(mgr + rm::EVICT_FROM, if g.below(2) == 0 { 0 } else { pick(g) });
    m.w32(mgr + rm::WRITING, if g.below(2) == 0 { 0 } else { pick(g) });
    m.w32(mgr + rm::MODE, g.below(14));
    m.w32(mgr + rm::RIDER, g.below(4));
    m.w32(mgr + rm::END, g.below(frame + 10));
    m.w32(mgr + rm::FRAME, g.below(frame + 10));
    m.w32(mgr + rm::PLAYING_ON, g.below(2));
    // highlights: (keyframe, length) records in H
    let mut recs = vec![];
    for i in 0..(H_LEN / 8) {
        let k = pick(g);
        m.w32(w.h + 8 * i, k);
        m.w32(w.h + 8 * i + 4, g.below(300));
        recs.push((w.h + 8 * i, k));
    }
    for rider in 0..4u32 {
        let c = g.below(4);
        m.w32(mgr + rm::HIGHLIGHT_COUNT + 4 * rider, c);
        for i in 0..c {
            let (h, _) = recs[g.below(recs.len() as u32) as usize];
            m.w32(mgr + rm::HIGHLIGHTS + rider * 0x28 + 4 * i, h);
        }
    }
    // the playlist
    let pl = mgr + 0x38ec;
    let c = 1 + g.below(4);
    m.w32(pl, c);
    m.w32(pl + 4, g.below(c));
    for i in 0..c {
        m.w32(pl + 8 + 4 * i, recs[g.below(recs.len() as u32) as usize].0);
    }
    m.w32(mgr + rm::PLAYLIST, if g.below(3) == 0 { pl } else { 0 });
    // a camera track: cuts in frame order
    let t = mgr + 0x5c;
    let mut cuts: Vec<(u32, u32)> = (0..g.below(6)).map(|_| recs[g.below(recs.len() as u32) as usize]).collect();
    cuts.sort_by_key(|&(_, k)| m.r32(k + kf::FRAME));
    m.w32(t, cuts.len() as u32);
    for (i, (h, _)) in cuts.iter().enumerate() {
        m.w32(t + 8 + 4 * i as u32, *h);
    }
    // pad streams
    for (rider, a) in w.inputs.iter().enumerate() {
        let on = g.below(2) == 0;
        m.w32(mgr + rm::INPUTS + 4 * rider as u32, if on { *a } else { 0 });
        let c = g.below(20);
        m.w32(a + 0x8000, c);
        for i in 0..c.max(1) {
            m.w8(a + 0x6664 + i, g.below(256) as u8);
        }
    }
    for (a, v) in &m.regions {
        r.mem.write(*a, v).unwrap();
    }
    m
}

fn call(r: &mut Runner, f: u32, args: &[u32]) -> u32 {
    for (k, a) in args.iter().enumerate() {
        r.cpu.set_gpr(4 + k, *a as i32 as i64 as u64);
    }
    r.call(f).unwrap_or_else(|e| panic!("{f:#x}: {e:?}"));
    r.cpu.gpr(2) as u32
}

#[test]
fn replay_keyframes_and_seeking_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [
        0x18eec0, 0x18efa0, 0x193068, 0x190f58, 0x190db0, 0x192758, 0x195440, 0x1955a0, 0x195460, 0x190e78, 0x190ee8,
        0x191100, 0x2cc808, 0x18ef20, 0x195428,
    ] {
        r.allow(a);
    }
    let w = World {
        mgr: r.mem.alloc(MGR_LEN),
        h: r.mem.alloc(H_LEN),
        inputs: [r.mem.alloc(IN_LEN), r.mem.alloc(IN_LEN), r.mem.alloc(IN_LEN), r.mem.alloc(IN_LEN)],
    };
    let mgr = w.mgr;
    let mut g = Rng(0x7265_706c_6179_0d01);
    let mut seen = [0u32; 32];
    let mut skipped = 0;
    for trial in 0..3000 {
        let mut m = build(&mut g, &mut r, &w);
        for step in 0..30 {
            let op = g.below(32) as usize;
            let what = format!("trial {trial} step {step} op {op}");
            let k = {
                let i = g.below(16);
                mgr + KEYFRAMES + 0x24 * i
            };
            let frame = if g.below(4) == 0 { g.next() >> g.below(32) } else { g.below(1500) };
            let from = if g.below(2) == 0 { 0 } else { k };
            // freeing is only asked of keyframes in the list (a free one twice makes the lists circular)
            let listed = {
                let mut v = vec![];
                let mut a = m.r32(mgr);
                while a != 0 && v.len() < 17 {
                    v.push(a);
                    a = m.r32(a + kf::NEXT);
                }
                v
            };
            if matches!(op, 4 | 5) && !listed.contains(&k) {
                continue;
            }
            let pl = mgr + 0x38ec;
            let t = mgr + 0x5c;
            let args: (u32, Vec<u32>) = match op {
                0 => (0x193068, vec![k]),
                1 => (0x18ee68, vec![mgr]),
                2 => (0x18eec0, vec![mgr, from]),
                3 => (0x18ef20, vec![mgr]),
                4 => (0x18efa0, vec![mgr, k]),
                5 => (0x191728, vec![mgr, k]),
                6 => (0x191778, vec![mgr, frame & 1]),
                7 => (0x191820, vec![mgr]),
                8 => (0x190f58, vec![mgr, frame & 3, k]),
                9 => (0x190d88, vec![mgr, frame % 20]),
                10 => (0x190db0, vec![mgr, frame]),
                11 => (0x190e78, vec![mgr, frame]),
                12 => (0x190ee8, vec![mgr, frame]),
                13 => (0x190ff0, vec![mgr]),
                14 => (0x191038, vec![mgr, k]),
                15 => (0x191078, vec![mgr]),
                16 => (0x18f020, vec![mgr]),
                17 => (0x18f0d8, vec![mgr]),
                18 => (0x18f168, vec![mgr]),
                19 => (0x191100, vec![mgr]),
                20 => (0x191190, vec![mgr]),
                21 => (0x1911e0, vec![mgr]),
                22 => (0x191278, vec![mgr]),
                23 => (0x195428, vec![pl]),
                24 => (0x1955a0, vec![pl]),
                25 => (0x195440, vec![pl]),
                26 => (0x195460, vec![pl]),
                27 => (0x195488, vec![pl]),
                28 => (0x1955b8, vec![pl, w.h + 8 * (frame % (H_LEN / 8))]),
                29 => (0x1955e0, vec![pl, frame % 4]),
                30 => (0x1954b8, vec![t, frame]),
                _ => (0x195510, vec![t, frame]),
            };
            let mut ours = m.clone();
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> (Option<u32>, Mem) {
                let m = &mut ours;
                let v = match op {
                    0 => {
                        rm::keyframe_reset(m, k);
                        None
                    }
                    1 => Some(rm::alloc_keyframe(m, mgr)),
                    2 => Some(rm::find_evictable_keyframe(m, mgr, from)),
                    3 => {
                        rm::evict_keyframe(m, mgr);
                        None
                    }
                    4 => {
                        rm::free_keyframe(m, mgr, k);
                        None
                    }
                    5 => {
                        rm::try_free_keyframe(m, mgr, k);
                        None
                    }
                    6 => {
                        rm::prune_keyframes(m, mgr, frame & 1 != 0);
                        None
                    }
                    7 => Some(rm::keyframe_before_latest(m, mgr)),
                    8 => Some(rm::is_keyframe_in_highlight(m, mgr, frame & 3, k) as u32),
                    9 => Some(rm::keyframe_at(m, mgr, frame % 20)),
                    10 => Some(rm::find_keyframe(m, mgr, frame)),
                    11 => Some(rm::next_keyframe(m, mgr, frame)),
                    12 => Some(rm::prev_keyframe(m, mgr, frame)),
                    13 => Some(rm::keyframe_count(m, mgr) as u32),
                    14 => Some(rm::keyframe_index(m, mgr, k) as u32),
                    15 => Some(rm::keyframe_count_used(m, mgr) as u32),
                    16 => Some(rm::is_at_end(m, mgr) as u32),
                    17 => {
                        rm::seek_start(m, mgr);
                        None
                    }
                    18 => {
                        rm::seek_end(m, mgr);
                        None
                    }
                    19 => {
                        rm::seek_to_keyframe(m, mgr);
                        None
                    }
                    20 => {
                        rm::step_forward(m, mgr);
                        None
                    }
                    21 => {
                        rm::step_back(m, mgr);
                        None
                    }
                    22 => {
                        rm::mode_10_to_11(m, mgr);
                        None
                    }
                    23 => Some(rm::playlist_current(m, pl)),
                    24 => Some(rm::playlist_current_keyframe(m, pl)),
                    25 => Some(rm::playlist_current_start(m, pl)),
                    26 => Some(rm::playlist_current_end(m, pl)),
                    27 => Some(rm::playlist_next(m, pl) as u32),
                    28 => {
                        rm::playlist_add(m, pl, w.h + 8 * (frame % (H_LEN / 8)));
                        None
                    }
                    29 => {
                        rm::playlist_set_current(m, pl, frame % 4);
                        None
                    }
                    30 => Some(rm::cam_track_seek_forward(m, t, frame) as u32),
                    _ => Some(rm::cam_track_seek_back(m, t, frame) as u32),
                };
                (v, m.clone())
            }));
            let Ok((v, after)) = res else {
                // our side stopped a walk that never ends: the game's never ends either
                for (k, a) in args.1.iter().enumerate() {
                    r.cpu.set_gpr(4 + k, *a as i32 as i64 as u64);
                }
                let e = r.call(args.0);
                assert!(matches!(e, Err(r5900::Error::TooLong { .. })), "{what}: the game finished: {e:?}");
                skipped += 1;
                break;
            };
            seen[op] += 1;
            let ret = call(&mut r, args.0, &args.1);
            if let Some(v) = v {
                assert_eq!(ret, v, "{what}: result");
            }
            m = after;
            let theirs = snapshot(&mut r, &w);
            for ((a, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
                if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
                    let i = i & !3;
                    panic!(
                        "{what}: memory at {:#x} (+{:#x} in the region at {a:#x}): {:x?} vs {:x?}",
                        a + i as u32,
                        i,
                        &x[i..(i + 4).min(x.len())],
                        &y[i..(i + 4).min(y.len())]
                    );
                }
            }
        }
    }
    eprintln!("ops {seen:?}, {skipped} trials ended in a walk that never ends (the game's too)");
}

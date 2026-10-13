//! The replay's highlights and pins (board row G12e) against the game in the function runner (TRICKY_ELF; skipped
//! without it): random managers with keyframes, the highlight pool and the riders' lists, and pins, built the same in
//! the runner and in our memory; random calls, every return and all the memory compared after each.

use std::cell::Cell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::replaymgr::{self as rm, hl, kf, pin, Mem};

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
const POOL: u32 = 0x3968;
const LOW: u32 = 0x100;
const GAPP: u32 = 0x337c58;

struct World {
    mgr: u32,
    pins: u32,
    app: u32,
    game: u32,
}

fn regions(w: &World) -> Vec<(u32, u32)> {
    vec![(0, LOW), (w.mgr, MGR_LEN), (w.pins, 0xe0), (w.app, 0x800), (w.game, 0x100), (GAPP, 4)]
}

fn snapshot(r: &mut Runner, w: &World) -> Mem {
    let mut m = Mem::default();
    for (a, n) in regions(w) {
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
        if *base == mgr || *base == w.pins {
            v.iter_mut().for_each(|b| *b = 0);
        }
    }
    m.w32(GAPP, w.app);
    m.w32(w.app + 0x730, w.game);
    m.w32(w.game + 0x40, mgr);
    // keyframes: a list in frame order and a free list
    let slot = |i: u32| mgr + KEYFRAMES + 0x24 * i;
    let mut order: Vec<u32> = (0..16).collect();
    for i in (1..16).rev() {
        order.swap(i, g.below(i as u32 + 1) as usize);
    }
    let n = 2 + g.below(15) as usize;
    let (list, free) = order.split_at(n);
    let mut frame = g.below(50);
    for (j, &i) in list.iter().enumerate() {
        let k = slot(i);
        m.w32(k + kf::FRAME, frame);
        frame += 1 + [0, 10, 30, 60, 200][g.below(5) as usize];
        m.w32(k + kf::PREV, if j > 0 { slot(list[j - 1]) } else { 0 });
        m.w32(k + kf::NEXT, if j + 1 < n { slot(list[j + 1]) } else { 0 });
        m.w32(k + kf::PINS, g.below(2));
        m.w32(k + kf::USED, (g.below(5) == 0) as u32);
    }
    for (j, &i) in free.iter().enumerate() {
        m.w32(slot(i) + kf::NEXT, if j + 1 < free.len() { slot(free[j + 1]) } else { 0 });
    }
    m.w32(mgr, slot(list[0]));
    m.w32(mgr + 4, if free.is_empty() { 0 } else { slot(free[0]) });
    m.w32(mgr + 0x10, slot(list[n - 1]));
    // the highlight pool: some in the riders' lists (in frame order, one per keyframe), the rest free
    let rec = |i: u32| mgr + POOL + 0x1c * i;
    let mut recs: Vec<u32> = (0..40).collect();
    for i in (1..40).rev() {
        recs.swap(i, g.below(i as u32 + 1) as usize);
    }
    let mut used = 0usize;
    let mut by_rider: Vec<Vec<u32>> = vec![vec![]; 4];
    for rider in 0..4u32 {
        let mut kfs: Vec<u32> = list.iter().map(|&i| slot(i)).filter(|_| g.below(3) == 0).collect();
        kfs.truncate(if g.below(8) == 0 { 10 } else { g.below(6) as usize });
        for (i, k) in kfs.iter().enumerate() {
            let h = rec(recs[used]);
            used += 1;
            m.w32(h + hl::KEYFRAME, *k);
            m.w32(h + hl::LEN, [0, 5, 30, 100, 400][g.below(5) as usize]);
            m.w32(h + hl::KIND, g.below(3));
            m.w32(h + hl::SCORE1, ((g.below(200) as f32 - 50.0) / 7.0).to_bits());
            m.w32(h + hl::SCORE2, ((g.below(200) as f32 - 50.0) / 3.0).to_bits());
            m.w32(h + hl::RIDER, rider);
            m.w32(mgr + rm::HIGHLIGHTS + rider * 0x28 + 4 * i as u32, h);
            by_rider[rider as usize].push(h);
        }
        m.w32(mgr + rm::HIGHLIGHT_COUNT + 4 * rider, kfs.len() as u32);
    }
    let rest = &recs[used..];
    for (j, &i) in rest.iter().enumerate() {
        m.w32(rec(i) + hl::NEXT_FREE, if j + 1 < rest.len() { rec(rest[j + 1]) } else { 0 });
    }
    m.w32(mgr + rm::HL_FREE, if rest.is_empty() { 0 } else { rec(rest[0]) });
    // the pins
    for rider in 0..4u32 {
        if g.below(4) == 0 {
            continue;
        }
        let p = w.pins + 0x1c * rider;
        m.w32(mgr + rm::PINS + 4 * rider, p);
        m.w32(p + pin::RIDER, rider);
        let mine = &by_rider[rider as usize];
        let any = |g: &mut Rng| if mine.is_empty() || g.below(3) == 0 { 0 } else { mine[g.below(mine.len() as u32) as usize] };
        let making = any(g);
        m.w32(p + pin::MAKING, making);
        for s in 0..4 {
            let h = any(g);
            m.w32(p + pin::SLOTS + 4 * s, if h == making { 0 } else { h });
        }
        m.w32(p + pin::KEYFRAME, if g.below(2) == 0 { 0 } else { slot(list[g.below(n as u32) as usize]) });
    }
    m.w32(mgr + rm::SLOT_COUNT, [2, 4][g.below(2) as usize]);
    m.w32(mgr + rm::QUOTA1, 1 + g.below(3));
    m.w32(mgr + rm::QUOTA2, 1 + g.below(2));
    m.w32(mgr + 0x40, (g.below(5) == 0) as u32);
    m.w32(mgr + 0x4c, (g.below(5) == 0) as u32);
    m.w32(mgr + 0x50, (g.below(5) == 0) as u32);
    m.w32(mgr + 0x2c, frame + g.below(100));
    m.w32(mgr + rm::MODE, [0, 0, 0, 1, 4, 9, 10, 11][g.below(8) as usize]);
    m.w32(mgr + rm::RIDER, g.below(4));
    m.w32(mgr + rm::END, frame + 10);
    let rider = m.r32(mgr + rm::RIDER) as usize;
    let fr = if g.below(5) == 0 { frame + 10 } else { g.below(frame + 20) };
    m.w32(mgr + rm::FRAME, fr);
    let cur = if by_rider[rider].is_empty() || g.below(2) == 0 { 0 } else { by_rider[rider][g.below(by_rider[rider].len() as u32) as usize] };
    m.w32(mgr + rm::RECORDING_HL, cur);
    m.w32(mgr + rm::RECORDING_FROM, if cur != 0 { m.r32(m.r32(cur) + kf::FRAME) + g.below(20) } else { 0 });
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
fn replay_highlights_and_pins_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [
        0x18efa0, 0x193068, 0x190f58, 0x190db0, 0x191728, 0x191820, 0x191eb0, 0x192058, 0x193ad0, 0x193a78, 0x1932f8,
        0x1932f0, 0x1931d0, 0x193250, 0x1932a8, 0x1934f0, 0x1935d8, 0x1937d0, 0x1938a0, 0x193aa0, 0x193ad8, 0x193118,
        0x193198, 0x193148, 0x2cc808, 0x192458, 0x1956c0,
    ] {
        r.allow(a);
    }
    // Replay_RecordKeyframe, MemPool_Alloc, Mem_Delete: what the test says
    let rec_ret = Rc::new(Cell::new(0u32));
    let rr = rec_ret.clone();
    r.stub(0x18f328, move |c, _| c.set_gpr(2, rr.get() as u64));
    let alloc_ret = Rc::new(Cell::new(0u32));
    let ar = alloc_ret.clone();
    r.stub(0x23d3b0, move |c, _| c.set_gpr(2, ar.get() as u64));
    let deleted = Rc::new(Cell::new(0u32));
    let dl = deleted.clone();
    r.stub(0x114f80, move |c, _| dl.set(c.gpr(4) as u32));
    let w = World { mgr: r.mem.alloc(MGR_LEN), pins: r.mem.alloc(0xe0), app: r.mem.alloc(0x800), game: r.mem.alloc(0x100) };
    let mgr = w.mgr;
    let mut g = Rng(0x6869_6768_6c69_0e01);
    let mut seen = [0u32; 26];
    // begun, inserted, dropped, slots picked, pins committed into a slot
    let mut hits = [0u32; 5];
    for trial in 0..3000 {
        let mut m = build(&mut g, &mut r, &w);
        for step in 0..25 {
            let op = g.below(26) as usize;
            let what = format!("trial {trial} step {step} op {op}");
            let rider = g.below(4);
            let frame = g.below(m.r32(mgr + rm::END) + 30);
            let h = mgr + POOL + 0x1c * g.below(40);
            let listed_h = {
                let n = m.r32(mgr + rm::HIGHLIGHT_COUNT + 4 * rider);
                if n == 0 { 0 } else { m.r32(mgr + rm::HIGHLIGHTS + rider * 0x28 + 4 * g.below(n)) }
            };
            let p = m.r32(mgr + rm::PINS + 4 * rider);
            let kind = g.below(3);
            let k = mgr + KEYFRAMES + 0x24 * g.below(16);
            let rec = if g.below(3) == 0 { 0 } else { m.r32(mgr) };
            rec_ret.set(rec);
            let new_pin = w.pins + 0x70 + 0x1c * g.below(4);
            alloc_ret.set(new_pin);
            // ops that need a pin skip when the rider has none
            if (9..=22).contains(&op) && op != 21 && p == 0 {
                continue;
            }
            let mut ours = m.clone();
            let mut del = 0u32;
            let v: Option<u32> = {
                let m = &mut ours;
                match op {
                    0 => Some(rm::insert_highlight(m, mgr, frame, rider)),
                    1 => {
                        rm::remove_highlight(m, mgr, rider, if listed_h != 0 { listed_h } else { h });
                        None
                    }
                    2 => Some(rm::find_highlight(m, mgr, rider, frame)),
                    3 => Some(rm::can_add_highlight(m, mgr)),
                    4 => Some(rm::begin_highlight(m, mgr, &mut |_, _| rec) as u32),
                    5 => {
                        rm::end_highlight(m, mgr);
                        None
                    }
                    6 => Some(rm::drop_current_highlight(m, mgr) as u32),
                    7 => {
                        rm::release_all_pins(m, mgr);
                        None
                    }
                    8 => {
                        rm::freeze(m, mgr);
                        None
                    }
                    9 => {
                        rm::pin_clear(m, mgr, p);
                        None
                    }
                    10 => {
                        rm::pin_release(m, mgr, p);
                        None
                    }
                    11 => {
                        rm::pin_begin(m, mgr, p, kind);
                        None
                    }
                    12 => {
                        rm::pin_end(m, mgr, p);
                        None
                    }
                    13 => {
                        rm::pin_commit(m, mgr, p);
                        None
                    }
                    14 => Some(rm::pin_pick_slot(m, mgr, p, if listed_h != 0 { listed_h } else { h }) as u32),
                    15 => Some(rm::pin_find_worst_slot(m, mgr, p, kind) as u32),
                    16 => Some(rm::pin_count_kind(m, p, kind) as u32),
                    17 => Some(rm::pin_quota(m, mgr, kind) as u32),
                    18 => {
                        rm::pin_clear_highlight_ref(m, p, listed_h);
                        None
                    }
                    19 => {
                        rm::pin_reset(m, p);
                        None
                    }
                    20 => {
                        rm::pin_init(m, p, rider);
                        None
                    }
                    22 => {
                        rm::remove_pin(m, mgr, p, &mut |_, q| del = q);
                        None
                    }
                    21 => Some(rm::add_pin(m, mgr, rider, &mut |_| new_pin)),
                    23 => {
                        rm::highlight_reset(m, h);
                        None
                    }
                    24 => {
                        rm::pin_set_length(m, mgr, h, frame);
                        None
                    }
                    _ => {
                        rm::keyframe_reset(m, k);
                        None
                    }
                }
            };
            seen[op] += 1;
            let (f, args): (u32, Vec<u32>) = match op {
                0 => (0x191eb0, vec![mgr, frame, rider]),
                1 => (0x192058, vec![mgr, rider, if listed_h != 0 { listed_h } else { h }]),
                2 => (0x192178, vec![mgr, rider, frame]),
                3 => (0x1921e0, vec![mgr]),
                4 => (0x1922a8, vec![mgr]),
                5 => (0x1924a0, vec![mgr]),
                6 => (0x1925f8, vec![mgr]),
                7 => (0x192458, vec![mgr]),
                8 => (0x191230, vec![mgr]),
                9 => (0x1931d0, vec![p]),
                10 => (0x1932f8, vec![p]),
                11 => (0x193370, vec![p, kind]),
                12 => (0x193448, vec![p]),
                13 => (0x1937d0, vec![p]),
                14 => (0x1935d8, vec![p, if listed_h != 0 { listed_h } else { h }]),
                15 => (0x1934f0, vec![p, kind]),
                16 => (0x1932a8, vec![p, kind]),
                17 => (0x193250, vec![p, kind]),
                18 => (0x1938a0, vec![p, listed_h]),
                19 => (0x193198, vec![p]),
                20 => (0x193118, vec![p, rider]),
                21 => (0x191a18, vec![mgr, rider]),
                22 => (0x191ac0, vec![mgr, p]),
                23 => (0x193a78, vec![h]),
                24 => (0x193ad8, vec![h, frame]),
                _ => (0x193068, vec![k]),
            };
            deleted.set(0);
            let ret = call(&mut r, f, &args);
            if let Some(v) = v {
                assert_eq!(ret, v, "{what}: result");
            }
            match (op, v) {
                (4, Some(1)) => hits[0] += 1,
                (0, Some(x)) if x != 0 => hits[1] += 1,
                (6, Some(1)) => hits[2] += 1,
                (14, Some(x)) if (x as i32) >= 0 => hits[3] += 1,
                (13, _) if ours_kept(&ours, p) => hits[4] += 1,
                _ => {}
            }
            if op == 22 {
                assert_eq!(deleted.get(), del, "{what}: the pin deleted");
            }
            m = ours;
            let theirs = snapshot(&mut r, &w);
            for ((a, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
                if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
                    let i = i & !3;
                    panic!(
                        "{what}: memory at {:#x}: {:x?} vs {:x?}",
                        a + i as u32,
                        &x[i..(i + 4).min(x.len())],
                        &y[i..(i + 4).min(y.len())]
                    );
                }
            }
        }
    }
    eprintln!("ops {seen:?} hits {hits:?}");
    assert!(hits.iter().all(|&h| h > 50));
}

/// A commit kept the highlight (it is no longer being made and sits in a slot)
fn ours_kept(m: &Mem, p: u32) -> bool {
    m.r32(p + pin::MAKING) == 0 && (0..4).any(|s| m.r32(p + pin::SLOTS + 4 * s) != 0)
}

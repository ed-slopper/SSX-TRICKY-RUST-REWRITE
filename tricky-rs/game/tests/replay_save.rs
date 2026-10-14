//! The replay's save to the card and the load (board row G12h) against the game in the function runner (TRICKY_ELF;
//! skipped without it): random managers saved chunk by chunk by `ReplaySave_Step` (every chunk, the writer's state
//! and all the memory compared after each), then the game's saved bytes loaded back by `Replay_LoadStream` on both
//! sides; and the career's replay buffer.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::replaymgr::{self as rm, hl, kf, pin, Mem};

mod replay_common;
use replay_common::*;

const CHUNK: u32 = 0x4000;
const SCRATCH: u32 = 0xc800;
const LOADBUF: u32 = 0x8_8000;

struct Extra {
    career: u32,
    buffer: u32,
    chunk: u32,
    scratch: u32,
    load: u32,
    cursor: u32,
    compress: u32,
    lit: u32,
}

fn all_regions(a: &Addrs, x: &Extra) -> Vec<(u32, u32)> {
    let mut v = regions(a);
    v.push((x.chunk, CHUNK));
    v.push((x.scratch, SCRATCH));
    v.push((x.load, LOADBUF));
    v.push((x.cursor, 16));
    v.push((x.career, 0x3720));
    v.push((x.buffer, 0x8_1000));
    v
}

fn snap(r: &mut Runner, a: &Addrs, x: &Extra) -> Mem {
    let mut m = Mem::default();
    for (at, n) in all_regions(a, x) {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

fn same(r: &mut Runner, a: &Addrs, x: &Extra, m: &Mem, what: &str) {
    let theirs = snap(r, a, x);
    for ((at, p), (_, q)) in theirs.regions.iter().zip(&m.regions) {
        if let Some(i) = (0..p.len()).find(|&i| p[i] != q[i]) {
            let i = i & !3;
            panic!("{what}: memory at {:#x} (+{i:#x} in the region at {at:#x}): {:x?} vs {:x?}", at + i as u32, &p[i..(i + 4).min(p.len())], &q[i..(i + 4).min(q.len())]);
        }
    }
}

/// A saveable manager: every kept slot filled (the game saves the unused ones' words from its stack), at most ten
/// highlights in all (a cleared manager has ten free), small keyframe streams, at most one pad stream
fn saveable(g: &mut Rng, m: &mut Mem, a: &Addrs) {
    let mgr = a.mgr;
    let race = m.r32(a.game + 0x1c);
    let riders = 1 + g.below(4);
    m.w32(race + 0x7c, riders);
    m.w32(race + 0x88, riders);
    let mut total = 0;
    for r in 0..4 {
        let at = mgr + rm::HIGHLIGHT_COUNT + 4 * r;
        let n = m.r32(at).min(10 - total);
        m.w32(at, n);
        total += n;
    }
    for r in 0..4 {
        let p = m.r32(mgr + rm::PINS + 4 * r);
        if p == 0 {
            continue;
        }
        let n = m.r32(mgr + rm::HIGHLIGHT_COUNT + 4 * r);
        if n == 0 {
            m.w32(mgr + rm::PINS + 4 * r, 0);
            continue;
        }
        for s in 0..4 {
            let h = m.r32(mgr + rm::HIGHLIGHTS + r * 0x28 + 4 * g.below(n));
            m.w32(p + pin::SLOTS + 4 * s, h);
        }
        m.w32(p + pin::MAKING, 0);
    }
    for i in 0..16 {
        let k = mgr + 0x3e68 + 0x24 * i;
        let base = a.streams + 0x40 * i;
        m.w32(k, base);
        // a keyframe always holds a snapshot (an empty block would not decode: G12c)
        m.w32(k + 0xc, 1 + g.below(0x40));
        let b = g.below(4) as u8;
        for j in 0..0x40 {
            m.w8(base + j, if g.below(3) == 0 { g.next() as u8 } else { b.wrapping_add((j / 8) as u8) });
        }
    }
    let keep = g.below(5);
    for r in 0..4 {
        if r != keep {
            m.w32(mgr + rm::INPUTS + 4 * r, 0);
        }
    }
    let _ = hl::KIND;
    let _ = kf::FRAME;
}

#[test]
fn replay_save_and_load_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.step_limit = 200_000_000;
    allow_replay(&mut r);
    for f in [0x166800, 0x166840, 0x166870, 0x1668a0] {
        r.allow(f);
    }
    let w = Rc::new(RefCell::new(World::default()));
    stub_world(&mut r, &w);
    let a = addrs(&mut r);
    let x = Extra {
        career: r.mem.alloc(0x3720),
        buffer: r.mem.alloc(0x8_1000),
        chunk: r.mem.alloc(CHUNK),
        scratch: r.mem.alloc(SCRATCH),
        load: r.mem.alloc(LOADBUF),
        cursor: r.mem.alloc(16),
        compress: r.mem.alloc(0x4000),
        lit: r.mem.alloc(0x10),
    };
    let mgr = a.mgr;
    let mut g = Rng(0x7361_7665_7265_0001);
    let mut chunks = 0;
    for trial in 0..12 {
        let dbg = std::env::var("SAVE_DEBUG").is_ok();
        if dbg { eprintln!("trial {trial}: build"); }
        let mut m0 = build(&mut g, &mut r, &a);
        if dbg { eprintln!("trial {trial}: saveable"); }
        saveable(&mut g, &mut m0, &a);
        for (at, v) in &m0.regions {
            r.mem.write(*at, v).unwrap();
        }
        let mut m = snap(&mut r, &a, &x);
        let world = |alloc_at: u32| World { alloc_at, compress_at: x.compress, rider_at: x.lit.wrapping_sub(0x5ae0), ..Default::default() };
        // ReplaySave_Init: the chunk then the scratch
        let mut ours = Ours(world(x.chunk));
        let mut seq = vec![x.chunk, x.scratch].into_iter();
        let mut env_init = SeqAlloc { inner: &mut ours, seq: &mut seq };
        if dbg { eprintln!("trial {trial}: save init"); }
        rm::save_init(&mut m, &mut env_init, mgr);
        let calls = ours.0.log.clone();
        let seq2 = Rc::new(RefCell::new(vec![x.chunk, x.scratch]));
        let s2 = seq2.clone();
        let ww = w.clone();
        r.stub(0x2ccf70, move |c, _| {
            ww.borrow_mut().log.push(Call::Alloc(c.gpr(5) as u32));
            let v = s2.borrow_mut().remove(0);
            c.set_gpr(2, v as u64)
        });
        *w.borrow_mut() = world(0);
        r.cpu.set_gpr(4, mgr as u64);
        r.call(0x193d98).unwrap();
        assert_eq!(w.borrow().log, calls, "trial {trial}: save init calls");
        same(&mut r, &a, &x, &m, &format!("trial {trial}: save init"));
        // the chunks
        let mut saved = vec![];
        for step in 0.. {
            let what = format!("trial {trial} chunk {step}");
            let mut ours = Ours(world(0));
            let v = rm::save_step(&mut m, &mut ours, mgr);
            *w.borrow_mut() = world(0);
            r.cpu.set_gpr(4, mgr as u64);
            r.cpu.set_gpr(5, x.cursor as u64);
            r.cpu.set_gpr(6, (x.cursor + 4) as u64);
            r.mem.write_u32(x.cursor, 0);
            r.mem.write_u32(x.cursor + 4, 0);
            m.w32(x.cursor, 0);
            m.w32(x.cursor + 4, 0);
            let t0 = std::time::Instant::now();
            r.call(0x193f70).unwrap_or_else(|e| panic!("{what}: {e:?}"));
            if std::env::var("SAVE_DEBUG").is_ok() {
                eprintln!("{what}: stage {} steps {} in {:?}", m.r32(mgr + 0x40c4), r.steps, t0.elapsed());
            }
            let ret = r.cpu.gpr(2) as u32;
            assert_eq!(ret, v.is_some() as u32, "{what}: result");
            let Some((buf, len)) = v else { break };
            assert_eq!((r.mem.read_u32(x.cursor), r.mem.read_u32(x.cursor + 4)), (buf, len), "{what}: chunk");
            m.w32(x.cursor, buf);
            m.w32(x.cursor + 4, len);
            assert_eq!(w.borrow().log, ours.0.log, "{what}: calls");
            same(&mut r, &a, &x, &m, &what);
            let mut b = vec![0u8; len as usize];
            r.mem.read(buf, &mut b).unwrap();
            saved.extend(b);
            chunks += 1;
        }
        assert_eq!(saved.len(), 0x7f6e0, "trial {trial}: the save's size");
        if dbg { eprintln!("trial {trial}: load"); }
        // load it back into a cleared manager
        rm::clear(&mut m, mgr);
        r.cpu.set_gpr(4, mgr as u64);
        r.call(0x18ebd0).unwrap();
        for (k, b) in saved.iter().enumerate() {
            m.w8(x.load + k as u32, *b);
        }
        r.mem.write(x.load, &saved).unwrap();
        m.w32(x.cursor, x.load);
        r.mem.write_u32(x.cursor, x.load);
        same(&mut r, &a, &x, &m, &format!("trial {trial}: before the load"));
        let via_buffer = trial % 2 == 1;
        let mut ours = Ours(world(0));
        let mut cur = x.load;
        let mut ok = true;
        if via_buffer {
            // the career's buffer: allocated, the race's set-up taken from it, the replay after it loaded, the buffer freed
            for k in 0..0x920 {
                m.w8(x.career + 0x27dc + k, g.next() as u8);
            }
            let mut seq = vec![x.buffer].into_iter();
            let mut e = SeqAlloc { inner: &mut ours, seq: &mut seq };
            rm::alloc_buffer(&mut m, &mut e, x.career);
            rm::load_rider_setup(&mut m, x.career);
            for (k, b) in saved.iter().enumerate() {
                m.w8(x.buffer + 0x920 + k as u32, *b);
            }
            let _ = &e;
            rm::load_from_buffer(&mut m, &mut e, x.career);
        } else {
            ok = rm::load_stream(&mut m, &mut ours, mgr, &mut cur);
        }
        if dbg { eprintln!("trial {trial}: ours loaded"); }
        m.w32(x.cursor, cur);
        *w.borrow_mut() = world(0);
        if via_buffer {
            for k in 0..0x920u32 {
                let b = m.r8(x.career + 0x27dc + k);
                r.mem.write(x.career + 0x27dc + k, &[b]).unwrap();
            }
            seq2.borrow_mut().push(x.buffer);
            r.cpu.set_gpr(4, x.career as u64);
            r.call(0x166800).unwrap();
            r.cpu.set_gpr(4, x.career as u64);
            r.call(0x166870).unwrap();
            r.mem.write(x.buffer + 0x920, &saved).unwrap();
            r.cpu.set_gpr(4, x.career as u64);
            r.call(0x1668a0).unwrap();
        } else {
            r.cpu.set_gpr(4, mgr as u64);
            r.cpu.set_gpr(5, x.cursor as u64);
            r.call(0x194910).unwrap();
            assert_eq!(r.cpu.gpr(2) as u32, ok as u32, "trial {trial}: load result");
        }
        assert_eq!(w.borrow().log, ours.0.log, "trial {trial}: load calls");
        same(&mut r, &a, &x, &m, &format!("trial {trial}: after the load"));
        assert!(ok, "trial {trial}: the rider count matches, so it loads");
        // and the save buffers freed
        let mut ours = Ours(world(0));
        rm::save_free(&mut m, &mut ours, mgr);
        *w.borrow_mut() = world(0);
        r.cpu.set_gpr(4, mgr as u64);
        r.call(0x194870).unwrap();
        assert_eq!(w.borrow().log, ours.0.log, "trial {trial}: free calls");
        same(&mut r, &a, &x, &m, &format!("trial {trial}: freed"));
    }
    eprintln!("{chunks} chunks compared");
}

/// `alloc` answering from a list (the save's two buffers)
struct SeqAlloc<'a, 'b> {
    inner: &'a mut Ours,
    seq: &'b mut std::vec::IntoIter<u32>,
}
impl tricky_game::replaymgr::CamEnv for SeqAlloc<'_, '_> {
    fn chase_params(&mut self) -> [u32; 5] {
        self.inner.chase_params()
    }
    fn preset(&mut self) -> u32 {
        self.inner.preset()
    }
    fn select_preset(&mut self, p: u32) {
        self.inner.select_preset(p)
    }
    fn set_chase_params(&mut self, c: [u32; 5], s: u32) {
        self.inner.set_chase_params(c, s)
    }
    fn set_replay_mode(&mut self, on: u32, r: u32) {
        self.inner.set_replay_mode(on, r)
    }
}
impl tricky_game::replaymgr::ReplayEnv for SeqAlloc<'_, '_> {
    fn menu_active(&mut self) -> bool {
        self.inner.menu_active()
    }
    fn broadphase_sort(&mut self) {
        self.inner.broadphase_sort()
    }
    fn contact_sort(&mut self) {
        self.inner.contact_sort()
    }
    fn save_keyframe(&mut self, m: &mut Mem, mgr: u32, k: u32, f: u32, fl: u32) {
        self.inner.save_keyframe(m, mgr, k, f, fl)
    }
    fn restore_keyframe(&mut self, m: &mut Mem, mgr: u32, k: u32) {
        self.inner.restore_keyframe(m, mgr, k)
    }
    fn hud_hide(&mut self, i: u32) {
        self.inner.hud_hide(i)
    }
    fn reset_rider_renders(&mut self) {
        self.inner.reset_rider_renders()
    }
    fn refresh_riders(&mut self) {
        self.inner.refresh_riders()
    }
    fn start_cut_fade(&mut self) {
        self.inner.start_cut_fade()
    }
    fn cam_script_stop(&mut self) {
        self.inner.cam_script_stop()
    }
    fn cancel_race_events(&mut self) {
        self.inner.cancel_race_events()
    }
    fn delete_nodes(&mut self, l: u32) {
        self.inner.delete_nodes(l)
    }
    fn world_reset_a(&mut self) {
        self.inner.world_reset_a()
    }
    fn world_reset_instances(&mut self) {
        self.inner.world_reset_instances()
    }
    fn alloc(&mut self, m: &mut Mem, size: u32) -> u32 {
        self.inner.alloc(m, size);
        self.seq.next().unwrap()
    }
    fn free(&mut self, m: &mut Mem, p: u32) {
        self.inner.free(m, p)
    }
    fn delete(&mut self, m: &mut Mem, p: u32) {
        self.inner.delete(m, p)
    }
}
impl tricky_game::replaymgr::SaveEnv for SeqAlloc<'_, '_> {
    fn update_lighting(&mut self, r: u32) {
        self.inner.update_lighting(r)
    }
    fn ea_free(&mut self, m: &mut Mem, p: u32) {
        self.inner.ea_free(m, p)
    }
}
